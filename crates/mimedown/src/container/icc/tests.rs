use super::*;
use crate::testing::icc_profile;

#[test]
fn a_profile_says_what_it_is_called_and_what_kind_it_is() {
  let profile = parse(&icc_profile(Some("Display P3"), false, b"RGB ")).unwrap();
  assert_eq!(profile.description.as_deref(), Some("Display P3"));
  assert_eq!(
    (profile.version.as_str(), profile.class.as_str()),
    ("2.1.0", "mntr")
  );
  assert_eq!(
    (
      profile.data_color_space.as_str(),
      profile.connection_space.as_str()
    ),
    ("RGB", "XYZ")
  );
  assert_eq!(
    (profile.cmm.as_deref(), profile.manufacturer.as_deref()),
    (Some("appl"), Some("APPL"))
  );
  assert_eq!(profile.created.as_deref(), Some("2026-11-17T23:36:27Z"));
  assert_eq!(profile.color_space(), ColorSpace::DisplayP3);
  assert_eq!(profile.name(), "Display P3");
}

#[test]
fn a_profile_of_the_later_version_keeps_its_name_in_utf16() {
  let profile = parse(&icc_profile(Some("sRGB IEC61966-2.1"), true, b"RGB ")).unwrap();
  assert_eq!(profile.description.as_deref(), Some("sRGB IEC61966-2.1"));
  assert_eq!(profile.version, "4.3.0");
  assert_eq!(profile.color_space(), ColorSpace::Srgb);
}

#[test]
fn a_profile_with_no_name_is_known_by_the_kind_of_color_it_describes() {
  let gray = parse(&icc_profile(None, false, b"GRAY")).unwrap();
  assert_eq!(
    (gray.description.as_deref(), gray.name().as_str()),
    (None, "embedded")
  );
  assert_eq!(gray.color_space(), ColorSpace::Gray);
  assert_eq!(
    parse(&icc_profile(None, false, b"CMYK"))
      .unwrap()
      .color_space(),
    ColorSpace::Cmyk
  );
  // Red, green and blue, and nothing to say which.
  assert_eq!(
    parse(&icc_profile(None, false, b"RGB "))
      .unwrap()
      .color_space(),
    ColorSpace::Unknown
  );
  let odd = parse(&icc_profile(Some("Camera input v3"), false, b"RGB ")).unwrap();
  assert_eq!(odd.color_space(), ColorSpace::Unknown);
}

#[test]
fn bytes_without_the_profiles_signature_are_not_one() {
  assert_eq!(parse(b"profile"), None);
  let mut near = icc_profile(Some("sRGB"), false, b"RGB ");
  near[36] = b'x';
  assert_eq!(parse(&near), None);
}
