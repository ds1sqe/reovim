mod common;
use common::*;

#[tokio::test]
async fn test_big_y_then_p() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("llYp") // Move to 'l', Y to yank "llo world", p to paste
        .run()
        .await;

    // Y yanks "llo world" (from cursor to end)
    // p should paste AFTER cursor position
    // Expected: "hello world" -> cursor at 'l' -> paste after -> "helllo worldlo world"
    eprintln!("Buffer: {}", result.buffer_content);
    eprintln!("Cursor: {:?}", result.cursor);
}

#[tokio::test]
async fn test_characterwise_yank_paste_after() {
    let result = ServerTest::new()
        .await
        .with_content("abc")
        .with_keys("yyp") // Yank 'a', move right, paste
        .run()
        .await;

    eprintln!("Buffer: {}", result.buffer_content);
    eprintln!("Cursor: {:?}", result.cursor);
}
