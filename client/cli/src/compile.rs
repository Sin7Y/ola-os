use std::{
    fs::File,
    io::Write,
    path::PathBuf,
};
use std::fs;

use anyhow::Error;
use anyhow::Result;
use num_bigint::BigInt;
use num_traits::Zero;
use ola_assembler::encoder::encode_asm_from_json_file;
use ola_lang::{
    abi,
    codegen::{core::ir::module::Module, isa::ola::Ola, lower::compile_module},
    file_resolver::FileResolver,
    sema::ast::{Layout, Namespace},
};

use crate::path::ExpandedPathbufParser;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Compile {
    #[clap(
        value_parser = ExpandedPathbufParser,
        help = "Path to ola source file"
    )]
    input: PathBuf,
    #[clap(
        value_parser = ExpandedPathbufParser,
        help = "Path to output dir"
    )]
    output_dir: PathBuf,
}

impl Compile {
    pub fn run(self) -> Result<()> {
        if !self.input.exists() {
            anyhow::bail!("ola file not found");
        }
        if self.output_dir.is_file() {
            anyhow::bail!("output dir is a file");
        }
        if !self.output_dir.exists() {
            let create_outdir = fs::create_dir_all(self.output_dir.clone());
            if let Err(err) = create_outdir {
                anyhow::bail!("create output dir failed: {}", err);
            }
        }
        let contract_name = self
            .input
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let asm_path = self.output_dir.join(contract_name.clone() + "_asm.json");
        let abi_path = self.output_dir.join(contract_name.clone() + "_abi.json");
        let bin_path = self.output_dir.join(contract_name + "_bin.json");

        let (asm_path, abi_path) = compile_ola_file_to_asm(
            self.input.display().to_string(),
            Some(asm_path.display().to_string()),
            Some(abi_path.display().to_string()),
        )?;
        let _ = ola_asm_to_binary(asm_path.clone(), Some(bin_path.display().to_string()));
        let _ = fs::remove_file(asm_path);
        Ok(())
    }
}

pub fn ola_asm_to_binary(asm_path: String, bin_path: Option<String>) -> Result<()> {
    let (_, mut bin_file) = generate_output_file(asm_path.clone(), "bin", bin_path)?;
    let program = match encode_asm_from_json_file(asm_path) {
        Ok(p) => p,
        Err(err) => anyhow::bail!("asm failed: {}", err),
    };
    let serialized = match serde_json::to_string(&program) {
        Ok(s) => s,
        Err(err) => anyhow::bail!("asm failed: {}", err),
    };
    let _ = bin_file.write_all(serialized.as_bytes());
    Ok(())
}

pub fn compile_ola_file_to_asm(
    ola_file_path: String,
    asm_path: Option<String>,
    abi_path: Option<String>,
) -> Result<(String, String)> {
    let (_src_path, mut ns) = pre_process_src(ola_file_path.clone())?;
    let (asm_path, asm_file) = generate_output_file(ola_file_path.clone(), "asm", asm_path)?;
    let (abi_path, abi_file) = generate_output_file(ola_file_path.clone(), "abi", abi_path)?;

    let _ = generate_asm(ola_file_path, &mut ns, asm_file)?;
    let _ = generate_abi(&mut ns, abi_file)?;
    Ok((
        asm_path.to_str().unwrap().to_string(),
        abi_path.to_str().unwrap().to_string(),
    ))
}

pub fn pre_process_src(ola_file_path: String) -> Result<(PathBuf, Namespace), Error> {
    let mut resolver = FileResolver::new();
    let src_path = match PathBuf::from(ola_file_path.clone()).canonicalize() {
        Ok(path) => path,
        Err(e) => anyhow::bail!("IO Error: {}", e),
    };
    match src_path.parent() {
        Some(parent) => {
            let _ = resolver.add_import_path(parent);
        }
        None => {
            anyhow::bail!("invalid source dir path")
        }
    }

    if let Err(e) = resolver.add_import_path(&PathBuf::from(".")) {
        anyhow::bail!("IO error: {}", e)
    }

    let mut ns = ola_lang::parse_and_resolve(ola_file_path.as_ref(), &mut resolver);
    if ns.diagnostics.any_errors() {
        ns.print_diagnostics(&resolver, true);
        anyhow::bail!("namespace error")
    }
    for contract_no in 0..ns.contracts.len() {
        layout(contract_no, &mut ns);
    }
    return Ok((src_path, ns));
}

fn layout(contract_no: usize, ns: &mut Namespace) {
    let mut slot = BigInt::zero();

    for var_no in 0..ns.contracts[contract_no].variables.len() {
        if !ns.contracts[contract_no].variables[var_no].constant {
            let ty = ns.contracts[contract_no].variables[var_no].ty.clone();

            ns.contracts[contract_no].layout.push(Layout {
                slot: slot.clone(),
                contract_no,
                var_no,
                ty: ty.clone(),
            });

            slot += ty.storage_slots(ns);
        }
    }

    ns.contracts[contract_no].fixed_layout_size = slot;
}

fn generate_abi(ns: &mut Namespace, mut output: File) -> Result<()> {
    for contract_no in 0..ns.contracts.len() {
        let (metadata, _) = abi::generate_abi(contract_no, &ns);
        match output.write_all(metadata.as_bytes()) {
            Ok(_) => {}
            Err(err) => anyhow::bail!("IO error: {}", err),
        }
    }
    Ok(())
}

fn generate_asm(src_name: String, ns: &mut Namespace, mut output: File) -> Result<()> {
    for contract_no in 0..ns.contracts.len() {
        let resolved_contract = &ns.contracts[contract_no];
        let context = inkwell::context::Context::create();
        let binary = resolved_contract.binary(&ns, &context, &src_name);
        // Parse the assembly and get a module
        let module = match Module::try_from(binary.module.to_string().as_str()) {
            Ok(m) => m,
            Err(err) => anyhow::bail!("IR parse failed: {}", err),
        };
        // Compile the module for Ola and get a machine module
        let isa = Ola::default();
        let code = match compile_module(&isa, &module) {
            Ok(c) => c,
            Err(err) => anyhow::bail!("Module compile failed: {}", err),
        };

        match output.write_all(format!("{}", code.display_asm()).as_bytes()) {
            Ok(_) => {}
            Err(err) => anyhow::bail!("IO Error: {}", err),
        }
    }
    Ok(())
}

fn generate_output_file(
    ola_file_path: String,
    file_type: &str,
    result_path: Option<String>,
) -> Result<(PathBuf, File)> {
    let output_path = if let Some(path) = result_path {
        PathBuf::from(path)
    } else {
        let mut path = PathBuf::from(&ola_file_path.clone());
        if let Some(stem) = path.file_stem() {
            path.set_file_name(stem.to_string_lossy().to_string() + "_" + file_type);
            path.set_extension("json");
        } else {
            anyhow::bail!("Invalid file path: {}", ola_file_path)
        }
        path
    };

    let output = match File::create(output_path.clone()) {
        Ok(f) => f,
        Err(err) => anyhow::bail!("IO Error: {}", err),
    };
    Ok((output_path, output))
}
