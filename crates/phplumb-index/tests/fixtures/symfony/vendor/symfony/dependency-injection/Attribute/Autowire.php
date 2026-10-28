<?php

namespace Symfony\Component\DependencyInjection\Attribute;

#[\Attribute(\Attribute::TARGET_PARAMETER)]
class Autowire
{
    public function __construct(
        string|array|null $value = null,
        ?string $service = null,
        ?string $expression = null,
        ?string $env = null,
        ?string $param = null,
        bool|string $lazy = false,
    ) {
    }
}
