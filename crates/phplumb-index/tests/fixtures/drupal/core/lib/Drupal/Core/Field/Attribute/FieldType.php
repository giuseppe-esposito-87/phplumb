<?php
namespace Drupal\Core\Field\Attribute;

#[\Attribute(\Attribute::TARGET_CLASS)]
class FieldType
{
    public function __construct(public readonly string $id)
    {
    }
}
