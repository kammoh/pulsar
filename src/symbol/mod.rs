use super::cbindings;
use std::error::Error;
use std::ffi::CString;

/**
 * Convert a symbolic reference to a file offset.  Accepted formats are:
 *    <offset>     - offset to file
 *    @<address>   - virtual address of a location
 *    <name>       - symbolic name, usually a function
 *    <src>:<line> - Start of a source line
 * All optionally followed by [+-]<offset>
 * 
 * returns: next (32-bit) word-aligned address
 **/
pub fn get_symbol_offset(file_name: &str, symbol: &str) -> Result<u64, Box<Error>> {
    let ret = unsafe {
        cbindings::sym_getsymboloffset(CString::new(file_name).unwrap().as_ptr() as *const i8,
         CString::new(symbol).unwrap().as_ptr() as *const i8)
    };

    if ret != !0 {
        // if ret & 3 != 0 {  
            // Ok( (ret & !3) + 4)
        // } else {
            Ok(ret)
        // }
    } else {
        Err(From::from("failed"))
    }
}

pub fn map_offset(file_name: &str, offset: u64) -> *const u8 {
    unsafe {
        cbindings::map_offset(CString::new(file_name).unwrap().as_ptr() as *const i8, offset) as *const u8
    }
}