use proc_macro::TokenStream;
use quote::quote;
use syn::{Attribute, Visibility, Ident, PatType, braced, parse::{ParseStream, Parse}, Token, FnArg, parenthesized, Pat, parse_macro_input, spanned::Spanned, Type, punctuated::Punctuated, MetaNameValue, Path, ItemImpl, ImplItem, TypePath, PathSegment, PathArguments, GenericArgument};
struct RpcProvider {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
    rpcs: Vec<RpcMethod>,
}
struct RpcMethod {
    attrs: Vec<Attribute>,
    ident: Ident,
    args: Vec<PatType>,
    output: Type,
}

impl Parse for RpcProvider {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis = input.parse()?;
        input.parse::<Token![trait]>()?;
        let ident: Ident = input.parse()?;
        let content;
        braced!(content in input);
        let mut rpcs = Vec::<RpcMethod>::new();
        while !content.is_empty() {
            rpcs.push(content.parse()?);
        }

        Ok(Self {
            attrs,
            vis,
            ident,
            rpcs,
        })
    }
}

impl Parse for RpcMethod {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        input.parse::<Token![async]>()?;
        input.parse::<Token![fn]>()?;
        let ident = input.parse()?;

        let content;
        parenthesized!(content in input);
        let mut args = Vec::new();

        for arg in content.parse_terminated(FnArg::parse, Token![,])? {
            match arg {
                FnArg::Typed(captured) if matches!(&*captured.pat, Pat::Ident(_)) => {
                    args.push(captured);
                }
                FnArg::Typed(_) => {
                    return Err(syn::Error::new(arg.span(), "patterns aren't allowed in RPC args"))
                }
                FnArg::Receiver(_) => {
                    return Err(syn::Error::new(arg.span(), "method args cannot start with self"))
                }
            }
        }

        input.parse::<Token![->]>()?;
        let output = input.parse()?;
        input.parse::<Token![;]>()?;

        Ok(Self {
            attrs,
            ident,
            args,
            output,
        })
    }
}

#[proc_macro_attribute]
pub fn rpc_provider(attr: TokenStream, input: TokenStream) -> TokenStream {
    let context = parse_macro_input!(attr as Type);

    let RpcProvider { attrs, ident, rpcs, vis } = parse_macro_input!(input as RpcProvider);

    let request_variants = rpcs.iter().map(|rpc| {
        let RpcMethod { attrs: _, ident, args, output: _ } = rpc;
        quote! {
            #[allow(non_camel_case_types)]
            #ident { #(#args),* }
        }
    });

    let response_variants = rpcs.iter().map(|rpc| {
        let RpcMethod { attrs: _, ident, args: _, output } = rpc;
        quote! {
            #[allow(non_camel_case_types)]
            #ident(#output)
        }
    });

    let request_enum_ident = syn::Ident::new(&format!("{}Request", ident), ident.span());
    let response_enum_ident = syn::Ident::new(&format!("{}Response", ident), ident.span());

    let rpc_call_body = rpcs.iter().map(|rpc| {
        let RpcMethod { attrs: _, ident, args, output: _ } = rpc;
        let untyped_args = args.iter().map(|a| &a.pat).collect::<Vec<_>>();
        quote! {
            #request_enum_ident::#ident { #(#untyped_args),* } => #response_enum_ident::#ident(self.#ident(ctx, #(#untyped_args),*).await)
        }
    });

    let rpc_server_fns = rpcs.iter().map(|rpc| {
        let RpcMethod { attrs, ident, args, output } = rpc;
        quote! {
            #(#attrs)*
            async fn #ident(&self, ctx: &#context, #(#args),*) -> anyhow::Result<#output>;
        }
    });

    let rpc_fn = quote! {
        async fn rpc_process(&self, ctx: &#context, msg: #request_enum_ident) -> anyhow::Result<#response_enum_ident> {
            match msg {
                #(#rpc_call_body),*
            }
        }
    };

    quote! {
        #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[serde(tag="method", content="data")]
        #vis enum #request_enum_ident {
            #(#request_variants),*
        }

        #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[serde(tag="method", content="data")]
        #vis enum #response_enum_ident {
            #(#response_variants),*
        }

        #(#attrs)*
        #[async_trait::async_trait]
        #vis trait #ident {
            #(#rpc_server_fns)*

            #rpc_fn
        }

        // #[async_trait::async_trait]
        // impl <T: #ident> RpcBase for T {
        //     type Context = #context;
        //     async fn rpc_call(&self, context: &#context, data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        //         serde_json::to_value(self.rpc_process(context, serde_json::from_value(&data).map_err(|e| anyhow::anyhow!(e))?).await).map_err(|e| anyhow::anyhow!(e))
        //     }
        // }
    }.into()
}

#[proc_macro_attribute]
pub fn rpc(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let t = parse_macro_input!(input as ItemImpl);

    let attrs = t.attrs;
    let generics = t.generics;
    let trait_ = match t.trait_ {
        Some((not, path, for_tok)) => match not {
            Some(not) => quote! { #not #path #for_tok },
            None => quote! { #path #for_tok }
        },
        None => quote!{}
    };
    let ty = match t.self_ty.as_ref() {
        Type::Path(path) => path,
        _ => Err("Not a path!").unwrap(),
    };

    let items = t.items.iter();

    let fns = t.items.iter().filter_map(|x| match x {
        ImplItem::Fn(f) => Some(f),
        _ => None
    });

    let fn_sig = fns.clone().map(|f| {
        let args = f.sig.inputs.iter().filter_map(|x| match x {
            FnArg::Receiver(_) => None,
            FnArg::Typed(t) => match t.pat.as_ref() {
                Pat::Ident(pident) => Some((pident, t.ty.clone())),
                _ => Err("Unsupported FnArg!").unwrap()
            }
        }).collect::<Vec<_>>();
        ( f.sig.ident.clone(), args, f.sig.output.clone(), f.sig.asyncness )
    });

    let request_variants = fn_sig.clone().map(|(fn_ident, args, _output, _asyncness)| {
        let args = args.iter().map(|(ident, ty)| quote!{ #ident: #ty });
        quote! {
            #[allow(non_camel_case_types)]
            #fn_ident { #(#args),* }
        }
    });
    
    let response_variants = fn_sig.clone().map(|(fn_ident, _args, output, _asyncness)| {
        let out = match output {
            syn::ReturnType::Default => quote! { },
            syn::ReturnType::Type(_arrow, t) => extract_type_from_result(&t).map(|x| quote!{ #x }).unwrap_or(quote!{ #t })
        };
        quote! {
            #[allow(non_camel_case_types)]
            #fn_ident(#out)
        }
    });

    let request_enum_ident = syn::Ident::new(&format!("{}Request", ty.path.segments.last().unwrap().ident), ty.span());
    let response_enum_ident = syn::Ident::new(&format!("{}Response", ty.path.segments.last().unwrap().ident), ty.span());

    let rpc_call_body = fn_sig.clone().map(|(fn_ident, args, _output, asyncness)| {
        let untyped_args = args.iter().map(|a| a.0.ident.clone()).collect::<Vec<_>>();

        let await_flag = asyncness.map(|_| quote! { .await }).unwrap_or(quote! {  });
        quote! {
            #request_enum_ident::#fn_ident { #(#untyped_args),* } => Ok(#response_enum_ident::#fn_ident(self.#fn_ident(#(#untyped_args),*)#await_flag?))
        }
    });

    let rpc_fn = quote! {
        pub async fn rpc_process(&self, msg: #request_enum_ident) -> anyhow::Result<#response_enum_ident> {
            match msg {
                #(#rpc_call_body),*
            }
        }
    };

    // TODO: These need to eject the inner type from anyhow::Result.
    quote! {
        #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[serde(tag="method", content="data")]
        pub enum #request_enum_ident {
            #(#request_variants),*
        }

        #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[serde(tag="method", content="data")]
        pub enum #response_enum_ident {
            #(#response_variants),*
        }

        #(#attrs)*
        impl #generics #trait_ #ty {
            #(#items)*

            #rpc_fn
        }

        #[async_trait::async_trait]
        impl RpcBase for #ty {
            async fn rpc_call(&self, data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
                serde_json::to_value(self.rpc_process(serde_json::from_value(data).map_err(|e| anyhow::anyhow!(e))?).await?).map_err(|e| anyhow::anyhow!(e))
            }
        }
    }.into()
}

/* Helpers */

// Adapted from https://stackoverflow.com/a/56264023
fn extract_type_from_result(ty: &syn::Type) -> Option<&syn::Type> {
  fn extract_type_path(ty: &syn::Type) -> Option<&Path> {
    match *ty {
      syn::Type::Path(ref typepath) if typepath.qself.is_none() => Some(&typepath.path),
      _ => None,
    }
  }

  fn extract_option_segment(path: &Path) -> Option<&PathSegment> {
    let idents_of_path = path
      .segments
      .iter()
      .into_iter()
      .fold(String::new(), |mut acc, v| {
        acc.push_str(&v.ident.to_string());
        acc.push('|');
        acc
      });
    vec!["Result|", "anyhow|Result|"]
      .into_iter()
      .find(|s| &idents_of_path == *s)
      .and_then(|_| path.segments.last())
  }

  extract_type_path(ty)
    .and_then(|path| extract_option_segment(path))
    .and_then(|path_seg| {
      let type_params = &path_seg.arguments;
      // It should have only on angle-bracketed param ("<String>"):
      match *type_params {
        PathArguments::AngleBracketed(ref params) => params.args.first(),
        _ => None,
      }
    })
    .and_then(|generic_arg| match *generic_arg {
      GenericArgument::Type(ref ty) => Some(ty),
      _ => None,
    })
}