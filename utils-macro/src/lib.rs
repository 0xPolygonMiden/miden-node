use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{parse_macro_input, parse_quote, Block, ItemFn};

#[proc_macro_attribute]
pub fn enable_logging(
    _attr: TokenStream,
    item: TokenStream,
) -> TokenStream {
    let mut function = parse_macro_input!(item as ItemFn);

    let name = function.sig.ident.to_string();
    let stmts = function.block.stmts;
    let block: Block = parse_quote! {{
        if ::std::env::args().any(|e| e == "--nocapture") {
            let subscriber = ::miden_node_utils::logging::subscriber::set_default(::miden_node_utils::logging::subscriber());
            let span = ::miden_node_utils::logging::span!(::tracing::Level::INFO, #name).entered();

            #(#stmts)*
        } else {
            #(#stmts)*
        };
    }};
    function.block = Box::new(block);

    function.into_token_stream().into()
}
