import { execFileSync } from 'node:child_process';

function run(command, args) {
  execFileSync(command, args, { stdio: 'inherit' });
}

if (process.env.WORKERS_CI === '1') {
  run('sh', ['-c', `
    set -eu
    zig_version=0.16.0
    zig_arch=\$(uname -m)
    case "\$zig_arch" in
      x86_64) zig_platform=x86_64-linux ;;
      aarch64|arm64) zig_platform=aarch64-linux ;;
      *) echo "Unsupported architecture: \$zig_arch" >&2; exit 1 ;;
    esac
    zig_dir="\$PWD/.zig"
    mkdir -p "\$zig_dir"
    curl -fsSL "https://ziglang.org/download/\$zig_version/zig-\$zig_platform-\$zig_version.tar.xz" \
      | tar -xJ --strip-components=1 -C "\$zig_dir"
    export PATH="\$zig_dir:\$PATH"
    zig version
  `]);
  run('sh', ['-c', 'curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.95.0']);
  run('sh', ['-c', 'PATH="$PWD/.zig:$HOME/.cargo/bin:$PATH" rustup target add wasm32-unknown-unknown --toolchain 1.95.0']);
  run('sh', ['-c', 'PATH="$PWD/.zig:$HOME/.cargo/bin:$PATH" rustup component add llvm-tools-preview --toolchain 1.95.0']);
  run('sh', ['-c', 'curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | PATH="$HOME/.cargo/bin:$PATH" sh']);

  const cargoBin = `${process.env.HOME}/.cargo/bin`;
  process.env.PATH = [`${process.cwd()}/.zig`, cargoBin, process.env.PATH].filter(Boolean).join(':');
  const rustHost = execFileSync('rustc', ['-vV'], { encoding: 'utf8' })
    .match(/^host: (.+)$/m)?.[1];
  const rustSysroot = execFileSync('rustc', ['--print', 'sysroot'], { encoding: 'utf8' }).trim();
  const llvmBin = rustHost ? `${rustSysroot}/lib/rustlib/${rustHost}/bin` : '';
  process.env.PATH = [
    `${process.cwd()}/.zig`,
    cargoBin,
    llvmBin,
    process.env.PATH,
  ].filter(Boolean).join(':');
}

run('pnpm', ['build']);
