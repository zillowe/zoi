project({
	name = "Zoi",
	config = {
		["local"] = true,
	},
})

registries({
	zoidberg = {
		url = "https://gitlab.com/zillowe/zillwen/zusty/zoidberg.git",
		revision = "main",
		type = "set",
	},
})

packages({
	"@zillowe/hello",
	"@zillowe/gct",
})

tasks({
	{
		cmd = "build",
		run = "rm -rf scripts/compiled && ./scripts/build.sh",
	},
	{
		cmd = "lines",
		run = "cloc crates",
	},
	{
		cmd = "deps",
		run = "cargo machete",
	},
	{
		cmd = "fmt",
		run = "cargo fmt --all",
	},
	{
		cmd = "lint",
		run = "cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged -- -D warnings",
	},
	{
		cmd = "check",
		run = "cargo check --workspace --all-targets",
	},
	{
		cmd = "test",
		run = "cargo test --all-features -- --test-threads=1",
	},
	{
		cmd = "speed",
		run = "hyperfine '~/.local/bin/zoi install @zillowe/zoko --dry-run' './dist/bin/zoi install @zillowe/zoko --dry-run'",
	},
	{
		cmd = "shell",
		run = "zoi dev",
	},
})

environments({
	{
		name = "Prepare",
		cmd = "pre",
		run = {
			"zoi run deps",
			"zoi run lint",
			"zoi run fmt",
			"zoi run check",
			"zoi run test",
		},
	},
})
