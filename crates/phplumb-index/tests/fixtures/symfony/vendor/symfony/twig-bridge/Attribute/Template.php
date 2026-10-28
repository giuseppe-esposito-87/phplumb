<?php

namespace Symfony\Bridge\Twig\Attribute;

#[\Attribute(\Attribute::TARGET_METHOD)]
class Template
{
    public function __construct(public string $template)
    {
    }
}
