#!/usr/bin/env node

import { spawn, exec } from "child_process";
import { platform, tmpdir } from "os";
import { createWriteStream } from "fs";
import { chmod } from "fs/promises";
import { join } from "path";
import { pipeline } from "stream/promises";

function isZoiInstalled(): Promise<boolean> {
  return new Promise((resolve) => {
    const command = process.platform === "win32" ? "where zoi" : "which zoi";
    exec(command, (error) => {
      if (error) {
        resolve(false);
      } else {
        resolve(true);
      }
    });
  });
}

async function installZoi() {
  const os = platform();
  let scriptUrl: string;
  let scriptName: string;
  let shell: string;
  let shellArgs: string[];

  if (os === "win32") {
    scriptUrl =
      "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/install.ps1";
    scriptName = "install.ps1";
    shell = "powershell.exe";
    shellArgs = ["-ExecutionPolicy", "Bypass", "-File"];
  } else if (os === "linux" || os === "darwin") {
    scriptUrl =
      "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/install.sh";
    scriptName = "install.sh";
    shell = "bash";
    shellArgs = [];
  } else {
    console.error(`Unsupported platform: ${os}`);
    process.exit(1);
  }

  const tempDir = tmpdir();
  const scriptPath = join(tempDir, scriptName);

  console.log(`Downloading zoi installer from ${scriptUrl}...`);

  try {
    const response = await fetch(scriptUrl);
    if (!response.ok || !response.body) {
      throw new Error(`Failed to download script: ${response.statusText}`);
    }

    await pipeline(response.body, createWriteStream(scriptPath));

    console.log(`Downloaded installer to ${scriptPath}`);

    if (os !== "win32") {
      await chmod(scriptPath, "755");
    }

    shellArgs.push(scriptPath);

    console.log(`Running installer with: ${shell} ${shellArgs.join(" ")}`);

    const child = spawn(shell, shellArgs, { stdio: "inherit" });

    child.on("error", (err) => {
      console.error(`Failed to start installer: ${err.message}`);
      process.exit(1);
    });

    child.on("exit", (code) => {
      if (code !== 0) {
        console.error(`Installer exited with code ${code}`);
      } else {
        console.log("Installation completed successfully.");
      }
      process.exit(code ?? 1);
    });
  } catch (error) {
    console.error("An error occurred during installation:", error);
    process.exit(1);
  }
}

async function main() {
  const installed = await isZoiInstalled();
  if (installed) {
    console.log("zoi is already installed. To upgrade, run 'zoi upgrade'.");
  } else {
    console.log("zoi not found, starting installation.");
    await installZoi();
  }
}

main();
