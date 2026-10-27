#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
include!(concat!(env!("OUT_DIR"), "/cbindings.rs"));

// Originally based on https://github.com/hexops/mach-dxcompiler/blob/main/src/main.zig
#[cfg(test)]
mod tests {
    #[test]
    fn mach_compile() {
        use super::*;
        use std::{ffi::CStr, ptr, slice, str};

        struct Compiler {
            handle: MachDxcCompiler,
        }

        impl Compiler {
            fn new() -> Self {
                Self {
                    handle: unsafe { machDxcInit() },
                }
            }

            fn compile<T, I>(&self, code: &[u8], args: I) -> CompileResult
            where
                T: AsRef<CStr>,
                I: IntoIterator<Item = T>,
            {
                let args = args
                    .into_iter()
                    .map(|s| s.as_ref().as_ptr())
                    .collect::<Vec<_>>();
                let mut options = MachDxcCompileOptions {
                    code: code.as_ptr().cast(),
                    code_len: code.len(),
                    args: args.as_ptr(),
                    args_len: args.len(),
                    include_callbacks: ptr::null_mut(),
                };
                unsafe { machDxcCompile(self.handle, &mut options) }.into()
            }
        }

        impl Drop for Compiler {
            fn drop(&mut self) {
                unsafe {
                    machDxcDeinit(self.handle);
                }
            }
        }

        struct CompileResult {
            handle: MachDxcCompileResult,
        }

        impl CompileResult {
            fn error(&self) -> Option<CompileError> {
                let handle = unsafe { machDxcCompileResultGetError(self.handle) };
                if handle.is_null() {
                    None
                } else {
                    Some(handle.into())
                }
            }

            fn object(&self) -> Option<CompileObject> {
                let handle = unsafe { machDxcCompileResultGetObject(self.handle) };
                if handle.is_null() {
                    None
                } else {
                    Some(handle.into())
                }
            }
        }

        impl From<MachDxcCompileResult> for CompileResult {
            fn from(handle: MachDxcCompileResult) -> Self {
                Self { handle }
            }
        }

        impl Drop for CompileResult {
            fn drop(&mut self) {
                unsafe {
                    machDxcCompileResultDeinit(self.handle);
                }
            }
        }

        struct CompileError {
            handle: MachDxcCompileError,
        }

        impl CompileError {
            fn message(&self) -> &str {
                let len = unsafe { machDxcCompileErrorGetStringLength(self.handle) };
                let ptr = unsafe { machDxcCompileErrorGetString(self.handle) };
                unsafe {
                    let bytes = slice::from_raw_parts(ptr.cast(), len);
                    str::from_utf8_unchecked(bytes)
                }
            }
        }

        impl From<MachDxcCompileError> for CompileError {
            fn from(handle: MachDxcCompileError) -> Self {
                Self { handle }
            }
        }

        impl Drop for CompileError {
            fn drop(&mut self) {
                unsafe {
                    machDxcCompileErrorDeinit(self.handle);
                }
            }
        }

        struct CompileObject {
            handle: MachDxcCompileObject,
        }

        impl CompileObject {
            fn bytes(&self) -> &[u8] {
                let len = unsafe { machDxcCompileObjectGetBytesLength(self.handle) };
                let ptr = unsafe { machDxcCompileObjectGetBytes(self.handle) };
                unsafe { std::slice::from_raw_parts(ptr.cast(), len) }
            }
        }

        impl From<MachDxcCompileObject> for CompileObject {
            fn from(handle: MachDxcCompileObject) -> Self {
                Self { handle }
            }
        }

        impl Drop for CompileObject {
            fn drop(&mut self) {
                unsafe {
                    machDxcCompileObjectDeinit(self.handle);
                }
            }
        }
        let code = r#"
        Texture1D<float4> tex[5] : register(t3);
        SamplerState SS[3] : register(s2);
    
        [RootSignature("DescriptorTable(SRV(t3, numDescriptors=5)), DescriptorTable(Sampler(s2, numDescriptors=3))")]
        float4 main(int i : A, float j : B) : SV_TARGET
        {
          float4 r = tex[NonUniformResourceIndex(i)].Sample(SS[NonUniformResourceIndex(i)], i);
          r += tex[NonUniformResourceIndex(j)].Sample(SS[i], j+2);
          return r;
        };"#;
        let args = &[
            CStr::from_bytes_until_nul(b"-E\0").unwrap(),
            CStr::from_bytes_until_nul(b"main\0").unwrap(),
            CStr::from_bytes_until_nul(b"-T\0").unwrap(),
            CStr::from_bytes_until_nul(b"ps_6_0\0").unwrap(),
            CStr::from_bytes_until_nul(b"-D\0").unwrap(),
            CStr::from_bytes_until_nul(b"MYDEFINE=1\0").unwrap(),
            CStr::from_bytes_until_nul(b"-Qstrip_debug\0").unwrap(),
            CStr::from_bytes_until_nul(b"-Qstrip_reflect\0").unwrap(),
        ];
        let compiler = Compiler::new();
        let result = compiler.compile(code.as_bytes(), args);
        if let Some(error) = result.error() {
            let msg = error.message();
            panic!("{msg}");
        }
        let Some(obj) = result.object() else {
            panic!("no object is produced");
        };
        assert_eq!(obj.bytes().len(), 2392);
    }
}
