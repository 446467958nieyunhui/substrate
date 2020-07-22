use super::{take_item_attrs, get_doc_literals};
use quote::ToTokens;
use syn::spanned::Spanned;

/// List of additional token to be used for parsing.
mod keyword {
	syn::custom_keyword!(DispatchResultWithPostInfo);
	syn::custom_keyword!(Call);
	syn::custom_keyword!(weight);
	syn::custom_keyword!(compact);
	syn::custom_keyword!(pallet);
}

/// Definition of dispatchables typically `impl<T: Trait> Call for Module<T> { ... }`
pub struct CallDef {
	/// A set of usage of instance, must be check for consistency with trait.
	pub instances: Vec<super::InstanceUsage>,
	/// The overal impl item.
	pub item: syn::ItemImpl,
	/// Information on methods (used for expansion).
	pub methods: Vec<CallVariantDef>,
	/// The keyword Call used (contains span).
	pub call: keyword::Call,
}

/// Definition of dispatchable typically: `#[weight...] fn foo(origin .., param1: ...) -> ..`
pub struct CallVariantDef {
	/// Function name.
	pub fn_: syn::Ident,
	/// Information on args: `(is_compact, name, type)`
	pub args: Vec<(bool, syn::Ident, Box<syn::Type>)>,
	/// Weight formula.
	pub weight: syn::Expr,
	/// Docs, used for metadata.
	pub docs: Vec<syn::Lit>,
}

/// Attributes for functions in call impl block.
/// Parse for `#[pallet::weight = expr]`
pub struct FunctionAttr {
	weight: syn::Expr,
}

impl syn::parse::Parse for FunctionAttr {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		input.parse::<syn::Token![#]>()?;
		let content;
		syn::bracketed!(content in input);
		content.parse::<keyword::pallet>()?;
		content.parse::<syn::Token![::]>()?;

		content.parse::<keyword::weight>()?;
		content.parse::<syn::Token![=]>()?;

		Ok(FunctionAttr {
			weight: content.parse::<syn::Expr>()?,
		})
	}
}

/// Attribute for arguments in function in call impl block.
/// Parse for `#[pallet::compact]|
pub struct ArgAttrIsCompact;

impl syn::parse::Parse for ArgAttrIsCompact {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		input.parse::<syn::Token![#]>()?;
		let content;
		syn::bracketed!(content in input);
		content.parse::<keyword::pallet>()?;
		content.parse::<syn::Token![::]>()?;

		content.parse::<keyword::compact>()?;
		Ok(ArgAttrIsCompact)
	}
}

impl CallDef {
	pub fn try_from(item: syn::Item) -> syn::Result<Self> {
		let mut item = if let syn::Item::Impl(item) = item {
			item
		} else {
			return Err(syn::Error::new(item.span(), "Invalid pallet::call, expect item impl"));
		};

		let mut instances = vec![];
		instances.push(super::check_impl_generics(&item.generics, item.impl_token.span())?);
		instances.push(super::check_module_usage(&item.self_ty)?);

		let call = item.trait_.take()
			.ok_or_else(|| {
				let msg = "Invalid pallet::call, expect Call ident as in \
					`impl<..> Call for Module<..> { .. }`";
				syn::Error::new(item.span(), msg)
			})?.1;
		let call = syn::parse2::<keyword::Call>(call.to_token_stream())?;

		let mut methods = vec![];
		for impl_item in &mut item.items {
			if let syn::ImplItem::Method(method) = impl_item {
				if method.sig.inputs.len() == 0 {
					let msg = "Invalid pallet::call, must have at least origin arg";
					return Err(syn::Error::new(method.sig.inputs.span(), msg));
				}
				super::check_dispatchable_first_arg(&method.sig.inputs[0])?;

				if let syn::ReturnType::Type(_, type_) = &method.sig.output {
					syn::parse2::<keyword::DispatchResultWithPostInfo>(type_.to_token_stream())?;
				} else {
					let msg = "Invalid pallet::call, require return type \
						DispatchResultWithPostInfo";
					return Err(syn::Error::new(method.sig.span(), msg));
				}

				let mut call_var_attrs: Vec<FunctionAttr> = take_item_attrs(&mut method.attrs)?;

				if call_var_attrs.len() != 1 {
					let msg = if call_var_attrs.len() == 0 {
						"Invalid pallet::call, require weight attribute i.e. `#[pallet::weight]`"
					} else {
						"Invalid pallet::call, to many weight attribute given"
					};
					return Err(syn::Error::new(method.sig.span(), msg));
				}
				let weight = call_var_attrs.pop().unwrap().weight;

				let mut args = vec![];
				for arg in method.sig.inputs.iter_mut().skip(1) {
					let arg = if let syn::FnArg::Typed(arg) = arg {
						arg
					} else {
						unreachable!("Only first argument can be receiver");
					};

					let arg_attrs: Vec<ArgAttrIsCompact> = take_item_attrs(&mut arg.attrs)?;

					if arg_attrs.len() > 1 {
						let msg = "Invalid pallet::call, argument has too many attributes";
						return Err(syn::Error::new(arg.span(), msg));
					}

					let arg_ident = if let syn::Pat::Ident(pat) = &*arg.pat {
						pat.ident.clone()
					} else {
						let msg = "Invalid pallet::call, argumen must be ident";
						return Err(syn::Error::new(arg.pat.span(), msg));
					};

					args.push((!arg_attrs.is_empty(), arg_ident, arg.ty.clone()));
				}

				let docs = get_doc_literals(&method.attrs);

				methods.push(CallVariantDef {
					fn_: method.sig.ident.clone(),
					weight,
					args,
					docs,
				});
			} else {
				let msg = "Invalid pallet::call, only method accepted";
				return Err(syn::Error::new(impl_item.span(), msg));
			}
		}

		Ok(Self {
			call,
			instances,
			item,
			methods
		})
	}
}

