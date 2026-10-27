<?php
namespace Drupal\gadget\Attribute;

#[\Attribute(\Attribute::TARGET_CLASS)]
class GadgetElement extends \Drupal\Component\Plugin\Attribute\Plugin
{
    public function __construct(public readonly string $id)
    {
    }
}
