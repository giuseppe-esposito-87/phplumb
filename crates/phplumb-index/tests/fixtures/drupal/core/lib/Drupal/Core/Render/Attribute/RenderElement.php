<?php
namespace Drupal\Core\Render\Attribute;

#[\Attribute(\Attribute::TARGET_CLASS)]
class RenderElement
{
    public function __construct(public readonly string $id)
    {
    }
}
