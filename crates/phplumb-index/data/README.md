# Engine data

`builtins.txt` lists the names of PHP's built-in classes, functions and constants. The engine uses it to tell a missing name from one that PHP or an extension provides, and to send the lookup of a built-in name to the stubs.

`stubs/` holds the stub files themselves: every PHP file of the same phpstorm-stubs tag outside its `tests` directory, with the project's `LICENSE`. The engine indexes them once per process, as PHP 8.5 has them, so a built-in class, function, constant or member leads to its stub declaration and its return type carries navigation on. The VS Code package ships them in `bin/stubs`, beside the engine binary.

Both come from [JetBrains phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs), Copyright JetBrains s.r.o., licensed under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0).
