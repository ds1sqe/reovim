use {
    super::{ExecBodyError, ExecBodyInnerFormat, looks_like_exec_body, parse_exec_body},
    crate::source_store::source_media_checksum32,
    reovim_testrt::{self as testrt, arch_test},
};

const BIN_SOURCE: &[u8] = b"reovim-source-v1\nexit-status ok\n";
const VALID_BIN_EXEC_BODY: &[u8] =
    b"reovim-exec-body-v1\ninner=bin-source-image\nbytes=32\nchecksum=4238279016\nreovim-source-v1\nexit-status ok\n";
const BAD_CHECKSUM_EXEC_BODY: &[u8] =
    b"reovim-exec-body-v1\ninner=bin-source-image\nbytes=32\nchecksum=1\nreovim-source-v1\nexit-status ok\n";
const BAD_LENGTH_EXEC_BODY: &[u8] =
    b"reovim-exec-body-v1\ninner=bin-source-image\nbytes=31\nchecksum=4238279016\nreovim-source-v1\nexit-status ok\n";
const BAD_INNER_EXEC_BODY: &[u8] =
    b"reovim-exec-body-v1\ninner=unknown\nbytes=32\nchecksum=4238279016\nreovim-source-v1\nexit-status ok\n";
const BIN_UAPI_BODY: &[u8] = b"reovim-bin-uapi-v1\nexit-status ok\n";

fn copy(out: &mut [u8], len: &mut usize, bytes: &[u8]) -> bool {
    if *len + bytes.len() > out.len() {
        return false;
    }
    out[*len..*len + bytes.len()].copy_from_slice(bytes);
    *len += bytes.len();
    true
}

fn copy_usize(out: &mut [u8], len: &mut usize, mut value: usize) -> bool {
    let mut digits = [0u8; 20];
    let mut count = 0usize;
    if value == 0 {
        digits[0] = b'0';
        count = 1;
    } else {
        while value > 0 {
            digits[count] = b'0' + (value % 10) as u8;
            value /= 10;
            count += 1;
        }
    }

    while count > 0 {
        count -= 1;
        if !copy(out, len, &digits[count..count + 1]) {
            return false;
        }
    }
    true
}

fn encode_exec_body(out: &mut [u8], inner: &[u8], body: &[u8]) -> usize {
    let mut len = 0usize;
    if !copy(out, &mut len, b"reovim-exec-body-v1\ninner=")
        || !copy(out, &mut len, inner)
        || !copy(out, &mut len, b"\nbytes=")
        || !copy_usize(out, &mut len, body.len())
        || !copy(out, &mut len, b"\nchecksum=")
        || !copy_usize(out, &mut len, source_media_checksum32(body) as usize)
        || !copy(out, &mut len, b"\n")
        || !copy(out, &mut len, body)
    {
        return 0;
    }
    len
}

arch_test!(exec_body_parser_accepts_checked_bin_source_body, {
    testrt::check(looks_like_exec_body(VALID_BIN_EXEC_BODY), "exec body magic is recognized");
    let parsed = parse_exec_body(VALID_BIN_EXEC_BODY).expect("valid exec body parses");
    testrt::check_eq(parsed.inner, ExecBodyInnerFormat::BinSourceImage);
    testrt::check_eq(parsed.inner.as_str(), "bin-source-image");
    testrt::check_eq(parsed.bytes, BIN_SOURCE);
    testrt::check_eq(parsed.checksum, source_media_checksum32(BIN_SOURCE));
});

arch_test!(exec_body_parser_accepts_bin_uapi_body, {
    let mut encoded = [0u8; 128];
    let len = encode_exec_body(&mut encoded, b"bin-uapi-v1", BIN_UAPI_BODY);
    testrt::check(len > 0, "bin uapi exec body fixture encodes");

    let parsed = parse_exec_body(&encoded[..len]).expect("valid bin uapi exec body parses");
    testrt::check_eq(parsed.inner, ExecBodyInnerFormat::BinUapiV1);
    testrt::check_eq(parsed.inner.as_str(), "bin-uapi-v1");
    testrt::check_eq(parsed.bytes, BIN_UAPI_BODY);
    testrt::check_eq(parsed.checksum, source_media_checksum32(BIN_UAPI_BODY));
});

arch_test!(exec_body_parser_rejects_bad_header_fields, {
    testrt::check_eq(
        parse_exec_body(b"reovim-source-v1\nexit-status ok\n"),
        Err(ExecBodyError::MissingMagic),
    );
    testrt::check_eq(parse_exec_body(BAD_INNER_EXEC_BODY), Err(ExecBodyError::InvalidInner));
    testrt::check_eq(parse_exec_body(BAD_LENGTH_EXEC_BODY), Err(ExecBodyError::BodyLengthMismatch));
    testrt::check_eq(parse_exec_body(BAD_CHECKSUM_EXEC_BODY), Err(ExecBodyError::ChecksumMismatch));
    testrt::check_eq(ExecBodyError::ChecksumMismatch.as_str(), "checksum-mismatch");
});
