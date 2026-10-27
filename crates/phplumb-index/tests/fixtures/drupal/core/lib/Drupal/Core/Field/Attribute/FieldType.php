<?php
namespace Drupal\Core\Field\Attribute;

#[\Attribute(\Attribute::TARGET_CLASS)]
class FieldType extends \Drupal\Component\Plugin\Attribute\Plugin
{
    public function __construct(public readonly string $id)
    {
    }
}
