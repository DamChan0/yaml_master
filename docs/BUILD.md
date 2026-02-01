# yed 빌드 가이드 (x86 / ARM, musl, Docker)

---

## Windows + Docker Desktop에서 빌드하기

Docker Desktop을 이미 설치했다면, 아래 순서대로 하면 됩니다.

### 준비

1. **Rust 설치**  
   [rustup](https://rustup.rs/)으로 설치 후 터미널에서 `cargo --version` 확인.
2. **Docker Desktop 실행**  
   트레이에서 Docker가 **Running** 상태인지 확인.

### 방법 A: cross 사용 (Linux x86 / ARM 모두 추천)

PowerShell 또는 CMD를 **관리자 권한 없이** 열고 프로젝트 폴더로 이동한 뒤:

```powershell
# 1. cross 설치 (최초 1회)
cargo install cross

# 2. Linux x86_64 musl 빌드
cross build --release --target x86_64-unknown-linux-musl
```

바이너리 위치: `target\x86_64-unknown-linux-musl\release\yed`

**ARM용도 빌드하려면:**

```powershell
# 64비트 ARM (Raspberry Pi 4/5 등)
cross build --release --target aarch64-unknown-linux-musl

# 32비트 ARM (Raspberry Pi 3 등)
cross build --release --target armv7-unknown-linux-musleabihf
```

- `target\aarch64-unknown-linux-musl\release\yed`
- `target\armv7-unknown-linux-musleabihf\release\yed`

**한 번에 여러 타깃 (PowerShell):**

```powershell
.\scripts\build-all.ps1 -LinuxMusl
```

위 스크립트는 x86_64 musl만 빌드합니다. ARM까지 한 번에 하려면 아래를 그대로 실행하면 됩니다.

```powershell
cross build --release --target x86_64-unknown-linux-musl
cross build --release --target aarch64-unknown-linux-musl
cross build --release --target armv7-unknown-linux-musleabihf
```

### 방법 B: Docker만 사용 (cross 없이)

`cross`를 쓰지 않고 Docker 이미지로만 빌드하는 방법입니다. **x86_64 Linux musl**만 만들어집니다.

PowerShell에서 프로젝트 루트로 이동한 뒤:

```powershell
# 1. 이미지 빌드
docker build -f docker/Dockerfile.musl -t yed-musl .

# 2. 컨테이너 실행 → 바이너리가 target 폴더로 복사됨
docker run --rm -v "${PWD}\target:/work/output" yed-musl
```

바이너리 위치: `target\x86_64-unknown-linux-musl\release\yed`

### Windows에서 자주 나오는 상황

| 증상 | 확인할 것 |
|------|------------|
| `error: linker 'cc' not found` 또는 toolchain 관련 오류 | **cross**를 쓰고 있는지 확인. Windows에서는 `cargo build --target x86_64-unknown-linux-musl`만 하면 실패함. `cross build ...` 사용. |
| `Cannot connect to the Docker daemon` | Docker Desktop이 실행 중인지 확인. 재시작 후 다시 시도. |
| `cross` 설치 실패 (Rust 버전) | `rustup update` 후 `cargo install cross` 다시 실행. |
| `toolchain 'stable-x86_64-unknown-linux-gnu' may not be able to run on this system` | Windows에서 cross가 Linux 툴체인을 쓰기 위해 필요함. 아래 명령 한 번 실행 후 다시 `cross build ...` 실행. |

**위 툴체인 오류 해결 (Windows):**

```powershell
rustup toolchain add stable-x86_64-unknown-linux-gnu --profile minimal --force-non-host
```

실행 후 다시 `cross build --release --target x86_64-unknown-linux-musl` 실행.

---

## 도커가 꼭 필요한가?

**아니요. 환경에 따라 다릅니다.**

| 상황 | 도커 필요 여부 |
|------|----------------|
| **Linux에서 x86_64 musl 빌드** | ❌ 불필요. `musl-tools` + `rustup target add` 만 있으면 됨 |
| **Linux에서 ARM용 크로스 빌드** | ✅ 권장. `cross`가 도커로 타깃 환경을 띄워서 빌드함 |
| **Windows/macOS에서 Linux용 빌드** | ✅ 필요. Linux 바이너리를 만들려면 Linux 환경(도커/VM/WSL 등)이 필요함 |

정리하면:

- **Linux x86_64 musl**: 도커 없이 네이티브로 빌드 가능.
- **Linux ARM (aarch64/armv7)**  
  - 해당 ARM 기기에서 직접 빌드하면 도커 불필요.  
  - x86/Windows/macOS 호스트에서 ARM용으로 크로스 빌드할 때는 `cross`(도커 사용)가 가장 편함.
- **Windows/macOS에서 Linux 바이너리**를 만들 때는 도커(또는 WSL/VM)가 사실상 필요함.

---

## 1. x86_64 Linux (musl) 빌드

### 1-1. Linux에서 도커 없이 (네이티브)

```bash
# musl 타깃 추가
rustup target add x86_64-unknown-linux-musl

# Ubuntu/Debian: musl 도구 설치
sudo apt-get install musl-tools

# Fedora
sudo dnf install musl-gcc

# Arch
sudo pacman -S musl

# 빌드 (정적 바이너리)
cargo build --release --target x86_64-unknown-linux-musl
```

결과: `target/x86_64-unknown-linux-musl/release/yed`

### 1-2. 도커로 x86_64 musl 빌드 (cross 사용)

Windows/macOS 또는 도커로 통일하고 싶을 때:

```bash
# cross 설치 (최초 1회)
cargo install cross

# 도커 실행 후
cross build --release --target x86_64-unknown-linux-musl
```

결과: `target/x86_64-unknown-linux-musl/release/yed`

### 1-3. 도커만 사용 (Dockerfile로 직접 빌드)

`cross` 없이, 같은 빌드를 도커 이미지 안에서만 하고 싶을 때:

```bash
# 프로젝트 루트에서 (Linux / macOS / WSL)
docker build -f docker/Dockerfile.musl -t yed-musl .
docker run --rm -v "${PWD}/target:/work/output" yed-musl
```

```powershell
# Windows PowerShell
docker build -f docker/Dockerfile.musl -t yed-musl .
docker run --rm -v "${PWD}\target:/work/output" yed-musl
```

결과: `target/x86_64-unknown-linux-musl/release/yed`

---

## 2. ARM Linux 빌드 (aarch64 / armv7)

### 2-1. ARM 기기에서 직접 빌드 (도커 불필요)

Raspberry Pi 4/5, ARM 서버 등에서:

```bash
git clone https://github.com/yourusername/yed.git
cd yed
cargo build --release
```

바이너리: `target/release/yed` (호스트가 aarch64면 aarch64용으로 빌드됨).

### 2-2. x86/Windows/macOS에서 ARM용 크로스 빌드 (도커 + cross)

**도커 실행 후** 다음만 실행하면 됨.

```bash
cargo install cross   # 최초 1회

# 64비트 ARM (Raspberry Pi 4/5, AWS Graviton 등)
cross build --release --target aarch64-unknown-linux-musl

# 32비트 ARM (Raspberry Pi 3 등)
cross build --release --target armv7-unknown-linux-musleabihf
```

결과:

- `target/aarch64-unknown-linux-musl/release/yed`
- `target/armv7-unknown-linux-musleabihf/release/yed`

---

## 3. 한 번에 여러 타깃 빌드 (스크립트)

### Linux / WSL / macOS (Bash)

```bash
./scripts/build-musl.sh
```

위 스크립트는 `cross`로 다음을 빌드함:

- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`
- `armv7-unknown-linux-musleabihf`

**도커가 켜져 있어야 합니다.**

### Windows (PowerShell)

Windows용 Linux musl 빌드:

```powershell
.\scripts\build-all.ps1 -LinuxMusl
```

역시 `cross` 사용 → 도커 필요.

---

## 4. 요약 표

| 목표 | 추천 방법 | 도커 |
|------|-----------|------|
| **Linux x86_64 musl** (Linux 호스트) | `rustup target add` + `musl-tools` 후 `cargo build --target x86_64-unknown-linux-musl` | 불필요 |
| **Linux x86_64 musl** (Windows/macOS 호스트) | `cross build --target x86_64-unknown-linux-musl` | 필요 |
| **Linux aarch64 musl** (어디서나) | `cross build --target aarch64-unknown-linux-musl` | 필요 (크로스 시) |
| **Linux armv7 musl** (어디서나) | `cross build --target armv7-unknown-linux-musleabihf` | 필요 (크로스 시) |
| **ARM 기기에서 직접** | 해당 기기에서 `cargo build --release` | 불필요 |

---

## 5. 정적 링크 확인 (Linux musl)

```bash
file target/x86_64-unknown-linux-musl/release/yed
# 예: ELF 64-bit LSB executable, x86-64, statically linked, ...

ldd target/x86_64-unknown-linux-musl/release/yed
# "not a dynamic executable" 이면 정적 링크된 것.
```

이 가이드대로면 x86/ARM, 도커 유무에 따라 필요한 방식만 골라서 쓰면 됩니다.
