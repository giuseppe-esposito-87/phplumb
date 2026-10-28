<?php

namespace Twig;

class Environment
{
    public function render(string $name, array $context = []): string
    {
        return '';
    }

    public function display(string $name, array $context = []): void
    {
    }

    public function load(string $name): object
    {
        return new \stdClass();
    }
}
