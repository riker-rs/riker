extern crate proc_macro;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{Generics, DeriveInput};

struct MsgTypes {
    types: Vec<MsgVariant>,
}

struct MsgVariant {
    name: Ident,
    mtype: Ident,
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
        // caused "missing extern crate token" compile error
        // let vars = Punctuated::<Ident, Token![,]>::parse_terminated(input)?;
        let vars = Punctuated::<Ident, syn::token::Comma>::parse_terminated(input)?;

        Ok(MsgTypes {
            types: vars
                .into_iter()
                .map(|t| MsgVariant {
                    name: get_name(&t),
                    mtype: t,
                })
                .collect::<Vec<_>>(),
        })
    }
}

fn get_name(ident: &Ident) -> Ident {
    let mut vname = format!("{}", ident.clone());

    if let Some(c) = vname.get_mut(0..1) {
        c.make_ascii_uppercase();
    }

    syn::Ident::new(&vname, ident.span())
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
    let intos = intos(&name, &types);
    let rec = receive(&ast.ident, &ast.generics, &name);

    let input: TokenStream = input.into();
    let gen = quote! {
        #input
        #menum
        #intos

        #rec
    };

    gen.into()
}

fn intos(name: &Ident, types: &MsgTypes) -> TokenStream {
    let intos = types
        .types
        .iter()
        .map(|t| impl_into(&name, &t.name, &t.mtype));
    quote! {
        #(#intos)*
    }
}

fn receive(aname: &Ident, gen: &Generics, name: &Ident) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = gen.split_for_impl();

    quote! {
        impl #impl_generics Receive<#name> for #aname #ty_generics #where_clause {
            type Msg = #name;

            fn receive(&mut self,
                        ctx: &Context<Self::Msg>,
                        msg: #name,
                        sender: Option<BasicActorRef>) {
                <#aname #ty_generics>::receive(self, ctx, msg, sender);
            }
        }
    }
}

fn impl_into(name: &Ident, vname: &Ident, ty: &Ident) -> TokenStream {
    quote! {
        impl Into<#name> for #ty {
            fn into(self) -> #name {
                #name::#vname(self)
            }
        }
    }
}
