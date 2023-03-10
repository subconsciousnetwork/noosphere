use std::collections::BTreeMap;
use std::io::Write;

#[derive(Debug)]
/// A writer for using with [safer_ffi::headers::builder] to
/// customize header generation. The following transformations
/// are applied:
///
/// Rust-to-C-case struct names (definitions, args, comments):
/// `typedef struct NsHeaders NsHeaders_t;`
/// `typedef struct ns_headers ns_headers_t;`
pub struct HeaderTransformer<W: Write> {
    inner: W,
    buffer: String,
    rust_classes: BTreeMap<String, String>,
}

impl<W: Write> HeaderTransformer<W> {
    pub fn new(inner: W) -> std::io::Result<Self> {
        Ok(HeaderTransformer {
            inner,
            buffer: String::with_capacity(200),
            rust_classes: BTreeMap::<String, String>::new(),
        })
    }

    /// This transformer uses two layers of buffered storage.
    /// The first layer `buffer` is a [String] holding incoming data
    /// until a semicolon is found (roughly a statement).
    /// This ensures sufficient context in this naive transformer,
    /// pushing the transformed text from the [String] buffer to
    /// the underlying `inner` buffer.
    fn transform_statement(&mut self) -> std::io::Result<usize> {
        let typedef_sigil = "typedef struct";
        let typedef_sigil_len = typedef_sigil.len();

        // Register new typedefs found.
        if let Some(index) = self.buffer.find("typedef struct") {
            let start_index = index + typedef_sigil_len + 1;
            let mut rust_name = String::with_capacity(16);
            for c in self.buffer[start_index..self.buffer.len()].chars() {
                if c == ' ' {
                    let c_name = rust_case_to_c_case(&rust_name);
                    self.rust_classes.insert(rust_name, c_name);
                    break;
                } else {
                    if !c.is_alphanumeric() {
                        // A non-alphanumeric character was parsed probably for
                        // a non-opaque struct, looks like:
                        // `typedef struct { size_t s } foo_t`
                        break;
                    }
                    rust_name.push(c);
                }
            }
        }

        // Replace rust class names with snake_case names.
        let mut line = String::with_capacity(self.buffer.len());
        std::mem::swap(&mut line, &mut self.buffer);

        // Reverse the iterator over the BTreeMap, which orders
        // by key, ensuring that class names that are subsets of
        // other class names are replaced appropriately.
        for (rust_name, c_name) in self.rust_classes.iter().rev() {
            line = line.replace(rust_name, c_name);
        }

        self.inner.write(line.as_bytes())
    }

    #[cfg(test)]
    pub fn into_inner(mut self) -> std::io::Result<W> {
        self.flush()?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for HeaderTransformer<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let utf8_buf = std::str::from_utf8(buf)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;

        let mut written = 0;
        for c in utf8_buf.chars() {
            self.buffer.push(c);
            written += 1;
            if c == ';' {
                self.transform_statement()?;
            }
        }

        // [safer_ffi::headers::builder::generate] never calls `flush` on
        // the provided writer, so look for the closing statement
        // in the header to flush the buffered data.
        if let Some(_) = self.buffer.rfind("#endif /* __RUST_NOOSPHERE__ */") {
            self.flush()?;
        }
        Ok(written)
    }

    /// This forces a transformation on intermediate storage,
    /// implying flush should only be called after full source
    /// text has been written.
    fn flush(&mut self) -> std::io::Result<()> {
        self.transform_statement()?;
        self.inner.flush()
    }
}

fn rust_case_to_c_case(class_name: &str) -> String {
    let mut out = String::with_capacity(class_name.len());
    for c in class_name.chars() {
        if c.is_uppercase() {
            if out.len() != 0 {
                out.push('_');
            }
            for l in c.to_lowercase() {
                out.push(l);
            }
        } else {
            out.push(c);
        }
    }
    return out;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn case_conversion() {
        assert_eq!(&rust_case_to_c_case("NsSphereFs_t"), "ns_sphere_fs_t");
        assert_eq!(&rust_case_to_c_case("URL_t"), "u_r_l_t"); // correct, but yikes
    }

    #[test]
    fn transforms_typedefs() {
        let buff = Cursor::new(Vec::<u8>::new());
        let mut transformer = HeaderTransformer::new(buff).unwrap();

        transformer
            .write("\n#ifndef __RUST_NOOSPHERE__\n".as_bytes())
            .unwrap();
        transformer
            .write("#define __RUST_NOOSPHERE__\n".as_bytes())
            .unwrap();

        // `NsSphere` defined before `NsSphereFile` tests that
        // token replacement works as expected when a token subset
        // exists as a class e.g. ensure that `NsSphere` does not
        // replace `NsSphereFile` as `ns_sphereFile`.
        transformer
            .write("typedef struct NsSphere NsSphere_t;\n".as_bytes())
            .unwrap();
        transformer
            .write("typedef struct NsSphereFile NsSphereFile_t;\n".as_bytes())
            .unwrap();

        transformer
            .write(
                r#"
void ns_sphere_file_foo (
NsSphereFile_t * file,
char const * name,
char const * value);"#
                    .as_bytes(),
            )
            .unwrap();
        transformer.write("\n".as_bytes()).unwrap();
        transformer.write("#endif ".as_bytes()).unwrap();
        transformer
            .write("/* __RUST_NOOSPHERE__ */\n".as_bytes())
            .unwrap();
        transformer.flush().unwrap();

        let buff: Cursor<Vec<u8>> = transformer.into_inner().unwrap();
        let inner_buff = buff.into_inner();
        let result = std::str::from_utf8(&inner_buff).unwrap();
        let expected = r#"
#ifndef __RUST_NOOSPHERE__
#define __RUST_NOOSPHERE__
typedef struct ns_sphere ns_sphere_t;
typedef struct ns_sphere_file ns_sphere_file_t;

void ns_sphere_file_foo (
ns_sphere_file_t * file,
char const * name,
char const * value);
#endif /* __RUST_NOOSPHERE__ */
"#;
        assert_eq!(result, expected);
    }
}
