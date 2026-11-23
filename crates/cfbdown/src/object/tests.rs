use super::*;

#[test]
fn a_packaged_file_is_given_with_its_name_and_no_more_than_its_own_bytes() {
  let mut data = vec![2, 0];
  data.extend(b"invoice.pdf\0C:\\in\\invoice.pdf\0");
  data.extend(3u32.to_le_bytes());
  data.extend(8u32.to_le_bytes());
  data.extend(b"C:\\tmp\\i\0");
  data.extend(4u32.to_le_bytes());
  data.extend(b"%PDFand what follows the file");
  let (name, bytes) = packaged(&data).unwrap();
  assert_eq!(name.as_deref(), Some("invoice.pdf"));
  assert_eq!(bytes, b"%PDF");
  // A length that runs past the data is not a packaged file.
  let cut = &data[..data.len() - 27];
  assert_eq!(packaged(cut), None);
  assert_eq!(packaged(b"\x02"), None);
}
