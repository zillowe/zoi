metadata({
  name = "test-split",
  repo = "core",
  version = "1.0.0",
  description = "Test split package",
  maintainer = { name = "Zoi", email = "zoi@example.com" },
  types = { "source" },
  sub_packages = { "dev", "lib" },
  main_subs = { "dev" },
})

function prepare()
  print("Preparing test-split")
end

function package()
  print("Packaging test-split")
end
