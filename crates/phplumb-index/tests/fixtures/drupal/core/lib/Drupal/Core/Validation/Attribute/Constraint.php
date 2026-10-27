<?php
namespace Drupal\Core\Validation\Attribute;

#[\Attribute(\Attribute::TARGET_CLASS)]
class Constraint
{
    public function __construct(public readonly string $id)
    {
    }
}
