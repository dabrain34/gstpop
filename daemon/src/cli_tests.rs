// cli_tests.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use super::Cli;
use clap::CommandFactory;

#[test]
fn verify_cli() {
    Cli::command().debug_assert();
}
