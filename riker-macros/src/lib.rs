extern crate proc_macro;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::{Colon2, Comma};
use syn::{DeriveInput, Generics, PathSegment, TypePath};

struct MsgTypes {
    types: Vec<MsgVariant>,
}

struct MsgVariant {
    name: Ident,
    mtype: TypePath,
}

impl MsgTypes {
    fn enum_stream(&self, name: &Ident) -> TokenStream {
        let vars = self.types.iter().map(|t| {
            let MsgVariant { name, mtype } = t;
            quote! {
                #name(#mtype),
            }
        });

        quote! {
            #[derive(Clone, Debug)]
            pub enum #name {
                #(#vars)*
            }
        }
    }
}

impl Parse for MsgTypes {
    fn parse(input: ParseStream) -> Result<Self> {
        let vars = Punctuated::<TypePath, Comma>::parse_terminated(input)?;

        Ok(MsgTypes {
            types: vars
                .into_iter()
                .map(|t| MsgVariant {
                    name: get_name(&t.path.segments),
                    mtype: t,
                })
                .collect::<Vec<_>>(),
        })
    }
}

fn get_name(segments: &Punctuated<PathSegment, Colon2>) -> Ident {
    let vname = segments
        .iter()
        .map(|seg| {
            let ident = format!("{}", seg.ident);
            ident
                .split('_')
                .map(|s| {
                    let mut s = s.to_string();
                    if let Some(c) = s.get_mut(0..1) {
                        c.make_ascii_uppercase();
                    }
                    s
                })
                .collect::<String>()
        })
        .collect::<String>();
    syn::Ident::new(&vname, segments.span())
}

#[proc_macro_attribute]
pub fn actor(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let i = input.clone();
    let ast = syn::parse_macro_input!(i as DeriveInput);

    let name = format!("{}Msg", ast.ident);
    let name = syn::Ident::new(&name, ast.ident.span());
    let types = syn::parse_macro_input!(attr as MsgTypes);

    let menum = types.enum_stream(&name);
    let froms = froms(&name, &types);
    let rec = receive(&ast.ident, &ast.generics, &name, &types);

    let input: TokenStream = input.into();
    let gen = quote! {
        #input
        #menum
        #froms

        #rec
    };

    gen.into()
}

fn froms(name: &Ident, types: &MsgTypes) -> TokenStream {
    let froms = types
        .types
        .iter()
        .map(|t| impl_from(&name, &t.name, &t.mtype));
    quote! {
        #(#froms)*
    }
}

fn receive(aname: &Ident, gen: &Generics, name: &Ident, types: &MsgTypes) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = gen.split_for_impl();

    let vars = types.types.iter().map(|t| {
        let vname = &t.name;
        let tname = &t.mtype;
        quote! {
            #name::#vname(msg) => <#aname #ty_generics as Receive<#tname>>::receive(self, ctx, msg, sender),
        }
    });

    quote! {
        impl #impl_generics Receive<#name> for #aname #ty_generics #where_clause {
            type Msg = #name;
            fn receive(&mut self,
                        ctx: &Context<Self::Msg>,
                        msg: Self::Msg,
                        sender: Option<BasicActorRef>) {
                match msg {
                    #(#vars)*
                }
            }
        }
    }
}

fn impl_from(name: &Ident, vname: &Ident, ty: &TypePath) -> TokenStream {
    quote! {
        impl From<#ty> for #name {
            fn from(obj: #ty) -> #name {
                #name::#vname(obj)
            }
        }
    }
}
