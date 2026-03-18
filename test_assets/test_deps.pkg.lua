metadata({
  name = "test-dep",
  repo = "core",
  version = "1.0.0",
  description = "Test dependencies",
  maintainer = { name = "Zoi", email = "zoi@example.com" },
  types = { "source" },
})

dependencies({
  build = {
    required = { "pacman:zip", "apt:zip" },
  },
})

function prepare()
  print("Preparing test-dep")
end

function package()
  print("Packaging test-dep")
  zmkdir("${pkgstore}/bin")
  cmd("echo 'echo hello' > ${pkgstore}/bin/test-dep")
end
