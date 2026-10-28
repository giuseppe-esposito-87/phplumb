<?php

namespace Doctrine\ORM\Mapping;

#[\Attribute(\Attribute::TARGET_CLASS)]
final class Entity
{
    public function __construct(public ?string $repositoryClass = null, public bool $readOnly = false)
    {
    }
}
