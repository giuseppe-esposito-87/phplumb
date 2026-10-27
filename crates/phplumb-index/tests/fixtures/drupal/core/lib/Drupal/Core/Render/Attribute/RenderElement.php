<?php
namespace Drupal\Core\Render\Attribute;

#[\Attribute(\Attribute::TARGET_CLASS)]
class RenderElement extends \Drupal\Component\Plugin\Attribute\Plugin
{
    public function __construct(public readonly string $id)
    {
    }
}
