<?php

namespace Symfony\Component\Routing\Attribute;

#[\Attribute(\Attribute::IS_REPEATABLE | \Attribute::TARGET_CLASS | \Attribute::TARGET_METHOD)]
class Route
{
    public function __construct(string|array|null $path = null, ?string $name = null)
    {
    }
}
