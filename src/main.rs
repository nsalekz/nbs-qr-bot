mod parser;
mod qr;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "nbs-qr-gen", about = "NBS IPS QR code generator - CLI & Telegram bot")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse raw text and generate QR code
    Gen {
        /// Raw payment text (invoice dump)
        text: String,

        /// Output file path
        #[arg(short = 'o', long, default_value = "nbs-qr.png")]
        output: PathBuf,

        /// QR image size in pixels
        #[arg(long, default_value = "512")]
        size: u32,
    },
    /// Run as Telegram bot
    Bot {
        /// Bot token (or set TELOXIDE_TOKEN env var)
        #[arg(long, env = "TELOXIDE_TOKEN")]
        token: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Gen { text, output, size } => {
            match parser::parse_payment(&text) {
                Ok(info) => {
                    println!("Parsed payment info:");
                    println!("  Payee:     {}", info.name);
                    println!("  Account:   {}", info.account);
                    println!("  Amount:    RSD {}", info.amount);
                    println!("  Purpose:   {}", info.purpose);
                    if let Some(ref r) = info.reference {
                        println!("  Reference: {}", r);
                    }
                    println!();

                    let payload = qr::build_ips_string(&info);
                    println!("IPS payload:\n{}\n", payload);

                    qr::generate_qr_image(&payload, &output, size)
                        .expect("Failed to generate QR");
                    println!("QR saved to: {}", output.display());
                }
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Bot { token } => {
            let token = token
                .or_else(|| std::env::var("TELOXIDE_TOKEN").ok())
                .expect("Bot token required: --token or TELOXIDE_TOKEN env var");
            bot::run(token).await;
        }
    }
}

mod bot {
    use crate::{parser, qr};
    use teloxide::prelude::*;
    use teloxide::types::InputFile;

    pub async fn run(token: String) {
        println!("Starting NBS QR bot...");

        let bot = Bot::new(token);

        teloxide::repl(bot, |bot: Bot, msg: Message| async move {
            let text = match msg.text() {
                Some(t) => t,
                None => {
                    bot.send_message(msg.chat.id, "Posalji mi tekst sa podacima za uplatu.")
                        .await?;
                    return Ok(());
                }
            };

            if text == "/start" {
                bot.send_message(
                    msg.chat.id,
                    "NBS IPS QR Generator\n\n\
                     Posalji mi podatke za uplatu (kopiraj ceo tekst sa fakture/poruke) \
                     i ja cu ti generisati QR kod za placanje.\n\n\
                     Potrebno mi je bar:\n\
                     - Broj racuna (npr. 160-445519-82)\n\
                     - Iznos\n\
                     - Naziv primaoca",
                )
                .await?;
                return Ok(());
            }

            match parser::parse_payment(text) {
                Ok(info) => {
                    let payload = qr::build_ips_string(&info);

                    let tmp = std::env::temp_dir().join(format!(
                        "nbs_qr_{}.png",
                        msg.chat.id
                    ));

                    match qr::generate_qr_image(&payload, &tmp, 512) {
                        Ok(()) => {
                            let caption = format!(
                                "Primalac: {}\nRacun: {}\nIznos: RSD {}\nSvrha: {}",
                                info.name, info.account, info.amount, info.purpose
                            );

                            bot.send_photo(msg.chat.id, InputFile::file(&tmp))
                                .caption(caption)
                                .await?;

                            let _ = std::fs::remove_file(&tmp);
                        }
                        Err(e) => {
                            bot.send_message(
                                msg.chat.id,
                                format!("Greska pri generisanju QR koda: {}", e),
                            )
                            .await?;
                        }
                    }
                }
                Err(e) => {
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "Nisam uspeo da procitam podatke.\n{}\n\n\
                             Potrebno mi je bar broj racuna (npr. 160-445519-82) i iznos.",
                            e
                        ),
                    )
                    .await?;
                }
            }

            Ok(())
        })
        .await;
    }
}
