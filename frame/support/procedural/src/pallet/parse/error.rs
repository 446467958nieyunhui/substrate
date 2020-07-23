use super::helper;
use syn::spanned::Spanned;
use quote::ToTokens;

/// List of additional token to be used for parsing.
mod keyword {
	syn::custom_keyword!(Error);
}

/// This checks error declaration as a enum declaration with only variants without fields nor
/// discriminant.
pub struct ErrorDef {
	/// The index of error item in pallet module.
	pub index: usize,
	/// Variants ident and doc literals (ordered as declaration order)
	pub variants: Vec<(syn::Ident, Vec<syn::Lit>)>,
	/// A set of usage of instance, must be check for consistency with trait.
	pub instances: Vec<helper::InstanceUsage>,
	/// The keyword error used (contains span).
	pub error: keyword::Error
}

impl ErrorDef {
	pub fn try_from(index: usize, item: &mut syn::Item) -> syn::Result<Self> {
		let item = if let syn::Item::Enum(item) = item {
			item
		} else {
			return Err(syn::Error::new(item.span(), "Invalid pallet::error, expect item enum"));
		};
		if !matches!(item.vis, syn::Visibility::Public(_)) {
			let msg = "Invalid pallet::error, `Error` must be public";
			return Err(syn::Error::new(item.span(), msg));
		}

		let mut instances = vec![];
		instances.push(helper::check_type_def_generics(&item.generics, item.span())?);

		if item.generics.where_clause.is_some() {
			let msg = "Invalid pallet::error, unexpected where clause";
			return Err(syn::Error::new(item.generics.where_clause.as_ref().unwrap().span(), msg));
		}

		let error = syn::parse2::<keyword::Error>(item.ident.to_token_stream())?;

		let variants = item.variants.iter()
			.map(|variant| {
				if !matches!(variant.fields, syn::Fields::Unit) {
					let msg = "Invalid pallet::error, unexpected fields, must be `Unit`";
					return Err(syn::Error::new(variant.fields.span(), msg));
				}
				if variant.discriminant.is_some() {
					let msg = "Invalid pallet::error, unexpected discriminant, discriminant \
						are not supported";
					let span = variant.discriminant.as_ref().unwrap().0.span();
					return Err(syn::Error::new(span, msg));
				}

				Ok((variant.ident.clone(), helper::get_doc_literals(&variant.attrs)))
			})
			.collect::<Result<_, _>>()?;

		Ok(ErrorDef {
			index,
			variants,
			instances,
			error,
		})
	}
}
