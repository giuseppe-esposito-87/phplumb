<?php

namespace Symfony\Component\Routing\Generator;

interface UrlGeneratorInterface
{
    public function generate(string $name, array $parameters = []): string;
}
