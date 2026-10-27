<?php
namespace Drupal\Core\Validation\Attribute;

#[\Attribute(\Attribute::TARGET_CLASS)]
class Constraint extends \Drupal\Component\Plugin\Attribute\Plugin
{
    public function __construct(public readonly string $id)
    {
    }
}
