//! Clipboard via OSC52 escape sequence (no dependency).
//! 通过 OSC52 转义序列写系统剪贴板（零依赖）。
//!
//! OSC52: a terminal escape code that asks the *terminal itself* to set the
//! clipboard. Works over SSH with no daemon — as long as your terminal
//! supports it (many do; some need it enabled in settings).
//! OSC52：一种终端转义码，让终端自己设置剪贴板。
//! 在 SSH 环境也能用，无需任何守护进程 — 前提是终端支持（多数支持）。
//!
//! Rust concepts introduced here:
//! 本文件涉及的 Rust 概念：
//! - byte strings `b"..."` and `&[u8]`: raw bytes, like C's `unsigned char*`.
//!   字节串 b"..." 与 &[u8]：原始字节，类似 C 的 unsigned char*。
//! - `let _ = ...`: deliberately discard a Result ("best effort, ignore
//!   errors"). The compiler warns about unused Results; `_ =` silences it.
//!   `let _ = ...`：故意忽略返回的 Result（尽力而为）。
//!   编译器会警告未处理的 Result，这样写表示明确丢弃。

use std::io::Write; // trait import: brings .write_all()/.flush() into scope.
                    // Rust has no methods by default on a generic handle — you
                    // import the TRAIT to get its methods. C++ analogue: ADL;
                    // C analogue: none, this is why the import exists.
                    // trait 导入：让 .write_all()/.flush() 可用。
                    // 泛型句柄的方法来自 trait，必须先导入 trait 才能调用其方法。

/// Copy text to the system clipboard via OSC52 (terminal-dependent;
/// harmless no-op on terminals that ignore it).
/// 用 OSC52 把文本复制到系统剪贴板（取决于终端；不支持的终端会安全忽略）。
pub(crate) fn copy_to_clipboard(s: &str) {
    // s.as_bytes(): view the string's UTF-8 bytes without copying.
    // s.as_bytes()：以字节视图访问字符串内容，不做拷贝。
    let b64 = base64_encode(s.as_bytes());

    // format! builds a String; \x1b = ESC, \x07 = BEL — same escapes as C.
    // format! 构造 String；\x1b 是 ESC，\x07 是 BEL — 与 C 相同的转义写法。
    let _ = std::io::stdout()
        .write_all(format!("\x1b]52;c;{b64}\x07").as_bytes());
    let _ = std::io::stdout().flush(); // push buffered bytes out now
                                       // 立即冲刷缓冲区，确保字节真正发出
}

/// Minimal base64 (RFC 4648) — no dependency.
/// 最小化 base64 实现（RFC 4648）— 不引第三方库。
///
/// Base64 packs every 3 bytes into 4 printable characters, so binary data
/// survives text-only channels (like terminal escape sequences).
/// base64 把每 3 字节编码为 4 个可打印字符，
/// 让二进制数据能走纯文本通道（如终端转义序列）。
pub fn base64_encode(data: &[u8]) -> String {
    // b"..." is a byte-string literal: a fixed-size array of u8 baked at
    // compile time. `&[u8; 64]` = reference to exactly 64 bytes.
    // b"..." 字节串字面量：编译期固定的 u8 数组。&[u8; 64] 指向恰好 64 字节。
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    // div_ceil(3): integer division rounding UP — how many output chars needed.
    // with_capacity pre-allocates so pushes never reallocate (like knowing
    // the size for malloc up front).
    // div_ceil(3)：整除并向上取整 — 需要的输出字符数。
    // with_capacity 预分配内存，push 时不再扩容（相当于提前定好 malloc 大小）。
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);

    // .chunks(3): iterator yielding &[u8] slices of length 1..=3 (last may be short).
    // No pointer arithmetic, no out-of-bounds possible.
    // .chunks(3)：产出长度 1..=3 的 &[u8] 切片迭代器（末尾可能不满）。
    // 无指针运算，不可能越界。
    for chunk in data.chunks(3) {
        // Reassemble 3 bytes into a 24-bit number: b0<<16 | b1<<8 | b2.
        // Missing trailing bytes become 0 (`unwrap_or(&0)`), and we emit '='
        // padding below instead of encoding them.
        // 把 3 字节拼成 24 位数。缺失的字节补 0，下面用 '=' 填充占位。
        // `chunk[0] as u32` widens u8->u32; `*chunk.get(1)` dereferences the
        // Option<&u8> that .get() returns (.get returns None instead of panicking).
        // chunk[0] as u32：u8 扩宽到 u32；.get(1) 返回 Option<&u8>，
        // 越界时给 None 而不是崩溃，unwrap_or(&0) 解引用取默认值。
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;

        // Split n into six 6-bit groups; each indexes the table T.
        // `as usize` converts to the index type; `as char` turns the table's
        // u8 back into a char (safe: table only holds ASCII).
        // 取出 6 个 6-bit 组中的对应组，作为查表下标。
        // as usize 转为下标类型；as char 把表内 u8 转回字符（表内全是 ASCII，安全）。
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(T[(n >> 6) as usize & 63] as char);
        } else {
            out.push('='); // padding: input had only 1 byte in this group
                           // 填充符：该组只有 1 字节输入
        }
        if chunk.len() > 2 {
            out.push(T[n as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// Unit tests — run with `cargo test`. assert_eq! prints both sides on failure.
// 单元测试 — cargo test 运行。assert_eq! 失败时会打印两边的值。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        // b"" etc.: byte-string literals compared against encoder output.
        // RFC 4648 test vectors: 0, 1, 2, 3, and 6 bytes.
        // RFC 4648 标准测试向量：0/1/2/3/6 字节的期望输出。
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }
}
