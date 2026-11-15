use napi_derive::napi;

// ── AcroForm field types (JS boundary) ──────────────────────────
//
// Read-only per-page form field extraction. `PageFormField` is a structured enum: napi-derive
// emits it as a TypeScript discriminated union keyed by `type`, one variant per field kind, every
// variant carrying the common fields plus its kind-specific ones. Buttons report the appearance
// state name (`on_state`) and the /Opt export value (`export_value`) separately. Presentation
// data (/DA, /Q, /MK, appearance streams) is deliberately not surfaced.

#[napi(object)]
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceOption {
  pub export_value: String,
  pub display_value: String,
}

/// A logical field with no widget placed on any page. Keeps the full field payload; only
/// geometry and per-widget state are absent.
#[napi(object)]
#[derive(Clone, Debug, PartialEq)]
pub struct UnplacedFormField {
  /// `"N G obj"`, or `"<parent fieldObjectId>/kid I"` / `"/Fields I"` when the field
  /// dictionary itself is written inline.
  pub field_object_id: String,
  pub name: String,
  pub partial_name: String,
  #[napi(js_name = "type", ts_type = "PageFormField[\"type\"]")]
  pub kind: String,
  pub value: Option<String>,
  /// Multi-select list boxes.
  pub values: Option<Vec<String>>,
  pub default_value: Option<String>,
  pub tooltip: Option<String>,
  pub mapping_name: Option<String>,
  /// Choice fields.
  pub options: Option<Vec<ChoiceOption>>,
  pub max_len: Option<u32>,
  pub read_only: bool,
  pub required: bool,
  pub no_export: bool,
  pub has_actions: bool,
  pub multiline: bool,
  pub password: bool,
  pub comb: bool,
  pub rich_text: bool,
  pub editable: bool,
  pub multi_select: bool,
  /// Signature fields: signature data present in /V (not cryptographic validation).
  pub signed: bool,
}

/// Document-level AcroForm facts. Present on `PdfMeta` / `PdfDocument` when `/AcroForm` exists
/// or orphan widgets were found.
#[napi(object)]
#[derive(Clone, Debug, PartialEq)]
pub struct FormInfo {
  /// `/AcroForm` present (false when only orphan widgets were found).
  pub has_acro_form: bool,
  /// `/AcroForm /XFA` present (XFA is not parsed).
  pub has_xfa: bool,
  pub need_appearances: bool,
  pub sig_flags: u32,
  /// Logical fields, placed + unplaced; may differ from `totalFormFields`.
  pub field_count: u32,
  /// Placements; always equals `totalFormFields` and `formFields.length`.
  pub widget_count: u32,
  pub unplaced_fields: Vec<UnplacedFormField>,
  /// The `/Kids` depth cap was hit somewhere: the result is partial.
  pub truncated: bool,
  /// Dangling references, non-array `/Annots`, non-dictionary `/Kids` entries, depth cap.
  pub warnings: Vec<String>,
}

/// Declares the common fields once and stamps them into every variant of the napi structured
/// enum, and stamps the conversion from the library's enum of the same shape.
macro_rules! page_form_field {
  // Kind-specific field types are written in brackets and forwarded as raw token trees: a `$ty:ty`
  // fragment reaches napi-derive wrapped in an invisible group, its `Option<…>` check misses, and a
  // `None` would be emitted as `null` instead of being omitted like every other optional field.
  ($($variant:ident { $($field:ident : [$($ty:tt)+]),* $(,)? }),+ $(,)?) => {
    #[napi(object_from_js = false)]
    pub enum PageFormField {
      $($variant {
        page: u32,
        rect: Vec<f64>,
        object_id: String,
        field_object_id: String,
        name: String,
        partial_name: String,
        widget_index: u32,
        widget_count: u32,
        value: Option<String>,
        default_value: Option<String>,
        tooltip: Option<String>,
        mapping_name: Option<String>,
        read_only: bool,
        required: bool,
        no_export: bool,
        has_actions: bool,
        $($field: $($ty)+,)*
      },)+
    }

    impl From<pdfdown::PageFormField> for PageFormField {
      fn from(field: pdfdown::PageFormField) -> Self {
        match field {
          $(pdfdown::PageFormField::$variant {
            page,
            rect,
            object_id,
            field_object_id,
            name,
            partial_name,
            widget_index,
            widget_count,
            value,
            default_value,
            tooltip,
            mapping_name,
            read_only,
            required,
            no_export,
            has_actions,
            $($field,)*
          } => PageFormField::$variant {
            page,
            rect,
            object_id,
            field_object_id,
            name,
            partial_name,
            widget_index,
            widget_count,
            value,
            default_value,
            tooltip,
            mapping_name,
            read_only,
            required,
            no_export,
            has_actions,
            $($field: $field.js(),)*
          },)+
        }
      }
    }
  };
}

page_form_field! {
  Text { multiline: [bool], password: [bool], comb: [bool], rich_text: [bool], max_len: [Option<u32>] },
  Checkbox { on_state: [Option<String>], export_value: [Option<String>], checked: [bool] },
  Radio { on_state: [Option<String>], export_value: [Option<String>], selected: [bool] },
  ComboBox { options: [Vec<ChoiceOption>], editable: [bool] },
  ListBox { options: [Vec<ChoiceOption>], multi_select: [bool], values: [Option<Vec<String>>] },
  PushButton {},
  Signature { signed: [bool] },
  Unknown {}
}

// ── Conversions ───────────────────────────────────────────────────

/// A library value as the shape JavaScript sees, for the kind-specific fields the macro stamps.
trait Js {
  type Out;
  fn js(self) -> Self::Out;
}

macro_rules! js_identity {
  ($($ty:ty),+) => {
    $(impl Js for $ty {
      type Out = $ty;
      fn js(self) -> $ty {
        self
      }
    })+
  };
}

js_identity!(bool, u32, String);

impl<T: Js> Js for Option<T> {
  type Out = Option<T::Out>;
  fn js(self) -> Self::Out {
    self.map(Js::js)
  }
}

impl<T: Js> Js for Vec<T> {
  type Out = Vec<T::Out>;
  fn js(self) -> Self::Out {
    self.into_iter().map(Js::js).collect()
  }
}

impl Js for pdfdown::ChoiceOption {
  type Out = ChoiceOption;
  fn js(self) -> ChoiceOption {
    self.into()
  }
}

impl From<pdfdown::ChoiceOption> for ChoiceOption {
  fn from(value: pdfdown::ChoiceOption) -> Self {
    ChoiceOption {
      export_value: value.export_value,
      display_value: value.display_value,
    }
  }
}

impl From<pdfdown::UnplacedFormField> for UnplacedFormField {
  fn from(value: pdfdown::UnplacedFormField) -> Self {
    UnplacedFormField {
      field_object_id: value.field_object_id,
      name: value.name,
      partial_name: value.partial_name,
      kind: value.kind,
      value: value.value,
      values: value.values,
      default_value: value.default_value,
      tooltip: value.tooltip,
      mapping_name: value.mapping_name,
      options: value
        .options
        .map(|v| v.into_iter().map(Into::into).collect()),
      max_len: value.max_len,
      read_only: value.read_only,
      required: value.required,
      no_export: value.no_export,
      has_actions: value.has_actions,
      multiline: value.multiline,
      password: value.password,
      comb: value.comb,
      rich_text: value.rich_text,
      editable: value.editable,
      multi_select: value.multi_select,
      signed: value.signed,
    }
  }
}

impl From<pdfdown::FormInfo> for FormInfo {
  fn from(value: pdfdown::FormInfo) -> Self {
    FormInfo {
      has_acro_form: value.has_acro_form,
      has_xfa: value.has_xfa,
      need_appearances: value.need_appearances,
      sig_flags: value.sig_flags,
      field_count: value.field_count,
      widget_count: value.widget_count,
      unplaced_fields: value.unplaced_fields.into_iter().map(Into::into).collect(),
      truncated: value.truncated,
      warnings: value.warnings,
    }
  }
}
