//! Values crossing the Fipe/wire boundary, with run-owned resources.

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

use fipe::ir::Tables;
use fipe::value::{Prelude, Record, Value};
use reovim_kernel_interface::{Calls, Error, Handle, Rd, Rights, Ty};
use reovim_sdk::dynamic::Value as Wire;
use reovim_sdk::ops;

pub struct Resources<'a> {
    pub ctx: &'a dyn Calls,
    handles: Vec<(Rd, Handle)>,
}

impl<'a> Resources<'a> {
    pub fn new(ctx: &'a dyn Calls) -> Self {
        Self { ctx, handles: Vec::new() }
    }

    pub fn received(&mut self, rd: Rd) -> Result<Value, Error> {
        if let Some(index) = self.handles.iter().position(|(at, _)| *at == rd) {
            return Ok(Value::Int(
                index.try_into().map_err(|_| Error::Invalid)?,
            ));
        }
        self.open(rd)
    }

    pub fn open(&mut self, rd: Rd) -> Result<Value, Error> {
        let index =
            i64::try_from(self.handles.len()).map_err(|_| Error::Invalid)?;
        let h = self.ctx.open(rd, Rights::READ.union(Rights::WRITE))?;
        self.handles.push((rd, h));
        Ok(Value::Int(index))
    }

    pub fn rd(&self, value: &Value) -> Result<Rd, Error> {
        Ok(self.entry(value)?.0)
    }

    pub fn handle(&self, value: &Value) -> Result<Handle, Error> {
        Ok(self.entry(value)?.1)
    }

    fn entry(&self, value: &Value) -> Result<&(Rd, Handle), Error> {
        let Value::Int(index) = value else { return Err(Error::Invalid) };
        let index = usize::try_from(*index).map_err(|_| Error::Invalid)?;
        self.handles.get(index).ok_or(Error::Invalid)
    }
}

impl Drop for Resources<'_> {
    fn drop(&mut self) {
        for (_, handle) in self.handles.drain(..) {
            ops::shut(self.ctx, &[handle]);
        }
    }
}

pub struct Mapping<'a, 'b> {
    pub tables: &'a Tables,
    pub home: &'a str,
    pub resources: &'a mut Resources<'b>,
}

impl Mapping<'_, '_> {
    pub fn lift(&mut self, ty: &Ty, value: Wire) -> Result<Value, Error> {
        let value = match (ty, value) {
            (Ty::Unit, Wire::Unit) => Value::Unit,
            (Ty::Bool, Wire::Bool(flag)) => {
                Prelude::new(self.tables).bool(flag)
            }
            (Ty::U8, Wire::U8(n)) => Value::Int(i64::from(n)),
            (Ty::U32, Wire::U32(n)) => Value::Int(i64::from(n)),
            (Ty::U64, Wire::U64(n)) => Value::integer(false, n.into()),
            (Ty::I64, Wire::I64(n)) => Value::Int(n),
            (Ty::F64, Wire::F64(n)) if n.is_finite() => Value::F64(n),
            (Ty::Utf8(_) | Ty::Rest(_), Wire::Utf8(s)) => Value::text(&s),
            (Ty::Bytes(_), Wire::Bytes(bytes)) => Value::bytes(&bytes),
            (Ty::Rd, Wire::Rd(rd)) => return self.resources.received(rd),
            (Ty::Array(_, item) | Ty::Sequence(_, item), Wire::Sequence(v)) => {
                let parts = v.into_iter().map(|v| self.lift(item, v));
                Value::seq(parts.collect::<Result<_, _>>()?)
            }
            (Ty::Record(name, fields), Wire::Record(values)) => {
                return self.lift_record(name, fields, values);
            }
            (Ty::Choice(name, fields), Wire::Choice(label, value)) => {
                return self.lift_choice(name, fields, &label, *value);
            }
            _ => return Err(Error::Invalid),
        };
        Ok(value)
    }

    pub fn to_wire(&self, ty: &Ty, value: &Value) -> Result<Wire, Error> {
        let invalid = |_| Error::Invalid;
        let value = match (ty, value) {
            (Ty::Unit, Value::Unit) => Wire::Unit,
            (Ty::Bool, Value::Record(record)) => self.boolean(record)?,
            (Ty::U8, Value::Int(n)) => {
                Wire::U8((*n).try_into().map_err(invalid)?)
            }
            (Ty::U32, Value::Int(n)) => {
                Wire::U32((*n).try_into().map_err(invalid)?)
            }
            (Ty::U64, value) => Wire::U64(unsigned(value)?),
            (Ty::I64, Value::Int(n)) => Wire::I64(*n),
            (Ty::F64, Value::F64(n)) if n.is_finite() => Wire::F64(*n),
            (Ty::Utf8(_) | Ty::Rest(_), Value::Text(s)) => {
                Wire::Utf8(s.as_ref().clone())
            }
            (Ty::Bytes(_), Value::Seq(values)) => {
                let bytes = values
                    .iter()
                    .map(|v| u8::try_from(unsigned(v)?).map_err(invalid));
                Wire::Bytes(bytes.collect::<Result<_, _>>()?)
            }
            (Ty::Rd, value) => Wire::Rd(self.resources.rd(value)?),
            (
                Ty::Array(_, item) | Ty::Sequence(_, item),
                Value::Seq(values),
            ) => {
                let parts = values.iter().map(|v| self.to_wire(item, v));
                Wire::Sequence(parts.collect::<Result<_, _>>()?)
            }
            (Ty::Record(name, fields), Value::Record(record)) => {
                self.lower_record(name, fields, record)?
            }
            (Ty::Choice(name, fields), Value::Record(record)) => {
                self.lower_choice(name, fields, record)?
            }
            _ => return Err(Error::Invalid),
        };
        // Bounds belong to the wire contract even when Fipe erased them.
        reovim_sdk::dynamic::encode(ty, &value)?;
        Ok(value)
    }

    fn lift_record(
        &mut self,
        name: &str,
        fields: &[reovim_kernel_interface::Field],
        values: Vec<(String, Wire)>,
    ) -> Result<Value, Error> {
        if fields.len() != values.len() {
            return Err(Error::Invalid);
        }
        let parts = fields
            .iter()
            .zip(values)
            .map(|(field, (n, v))| {
                (field.name == n).then_some(()).ok_or(Error::Invalid)?;
                self.lift(&field.ty, v)
            })
            .collect::<Result<_, _>>()?;
        self.record(name, 0, parts)
    }

    fn lift_choice(
        &mut self,
        name: &str,
        fields: &[reovim_kernel_interface::Field],
        label: &str,
        value: Wire,
    ) -> Result<Value, Error> {
        let index = fields
            .iter()
            .position(|f| f.name == label)
            .ok_or(Error::Invalid)?;
        let value = self.lift(&fields[index].ty, value)?;
        let meaning = self.meaning(name)?;
        let constructor = self.tables.meanings[meaning]
            .constructors
            .get(index)
            .ok_or(Error::Invalid)?;
        let values = if constructor.fields.is_empty() && value == Value::Unit {
            Vec::new()
        } else {
            alloc::vec![value]
        };
        self.record(name, index, values)
    }

    fn boolean(&self, record: &Record) -> Result<Wire, Error> {
        if Some(record.meaning) != Prelude::new(self.tables).bool
            || record.constructor > 1
            || !record.fields.is_empty()
        {
            return Err(Error::Invalid);
        }
        Ok(Wire::Bool(record.constructor == 0))
    }

    fn lower_record(
        &self,
        name: &str,
        fields: &[reovim_kernel_interface::Field],
        record: &Record,
    ) -> Result<Wire, Error> {
        self.check_record(name, record)?;
        if record.constructor != 0 || fields.len() != record.fields.len() {
            return Err(Error::Invalid);
        }
        let values = fields
            .iter()
            .zip(&record.fields)
            .map(|(f, v)| Ok((f.name.clone(), self.to_wire(&f.ty, v)?)))
            .collect::<Result<_, Error>>()?;
        Ok(Wire::Record(values))
    }

    fn lower_choice(
        &self,
        name: &str,
        fields: &[reovim_kernel_interface::Field],
        record: &Record,
    ) -> Result<Wire, Error> {
        self.check_record(name, record)?;
        let field = fields.get(record.constructor).ok_or(Error::Invalid)?;
        let value = match record.fields.as_slice() {
            [] if field.ty == Ty::Unit => Wire::Unit,
            [value] => self.to_wire(&field.ty, value)?,
            _ => return Err(Error::Invalid),
        };
        Ok(Wire::Choice(field.name.clone(), Box::new(value)))
    }

    fn meaning(&self, name: &str) -> Result<usize, Error> {
        let (api, label) = name.rsplit_once('.').ok_or(Error::Invalid)?;
        let namespace = if self.home.is_empty() {
            api.into()
        } else {
            format!("{}.{api}", self.home)
        };
        self.tables
            .meanings
            .iter()
            .position(|m| {
                m.label == label
                    && self.tables.namespaces[m.namespace] == namespace
            })
            .ok_or(Error::Invalid)
    }

    fn record(
        &self,
        name: &str,
        constructor: usize,
        fields: Vec<Value>,
    ) -> Result<Value, Error> {
        Ok(Value::Record(Rc::new(Record {
            meaning: self.meaning(name)?,
            constructor,
            fields,
        })))
    }

    fn check_record(&self, name: &str, record: &Record) -> Result<(), Error> {
        if record.meaning != self.meaning(name)? {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}

fn unsigned(value: &Value) -> Result<u64, Error> {
    match value {
        Value::Int(n) => (*n).try_into().map_err(|_| Error::Invalid),
        Value::Big(n) if !n.negative => {
            n.magnitude.to_u64().ok_or(Error::Invalid)
        }
        _ => Err(Error::Invalid),
    }
}

#[cfg(test)]
mod tests;
