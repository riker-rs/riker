extern crate proc_macro;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{
    DeriveInput, Generics, QSelf, Type, TypeArray, TypeGroup, TypeParen, TypePath, TypePtr,
    TypeReference, TypeSlice, TypeTuple,
};

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
                    name: Ident::new(&get_path_name(&t), t.span()),
                    mtype: t,
                })
                .collect::<Vec<_>>(),
        })
    }
}

fn get_type_name(ty: &Type) -> String {
    match ty {
        Type::Path(path) => get_path_name(path),
        Type::Array(TypeArray { elem, .. })
        | Type::Group(TypeGroup { elem, .. })
        | Type::Paren(TypeParen { elem, .. })
        | Type::Ptr(TypePtr { elem, .. })
        | Type::Reference(TypeReference { elem, .. })
        | Type::Slice(TypeSlice { elem, .. }) => get_type_name(elem.as_ref()),
        Type::Tuple(TypeTuple { elems, .. }) => {
            elems.iter().fold(String::new(), |mut acc, curr| {
                acc.push_str(&get_type_name(&curr));
                acc
            })
        }
        _ => unreachable!(),
    }
}

fn get_path_name(type_path: &TypePath) -> String {
    let mut vname = if let Some(QSelf { ty, .. }) = &type_path.qself {
        get_type_name(ty.as_ref())
    } else {
        String::new()
    };
    vname.push_str(
        &type_path
            .path
            .segments
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
            .collect::<String>(),
    );
    vname
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
    let rec = receive(&ast.ident, &ast.generics, &name, &types);

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

fn impl_into(name: &Ident, vname: &Ident, ty: &TypePath) -> TokenStream {
    quote! {
        impl Into<#name> for #ty {
            fn into(self) -> #name {
                #name::#vname(self)
            }
        }
    }
}
