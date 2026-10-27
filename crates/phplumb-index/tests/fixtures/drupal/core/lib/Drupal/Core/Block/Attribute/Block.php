<?php
namespace Drupal\Core\Block\Attribute;

#[\Attribute(\Attribute::TARGET_CLASS)]
class Block
{
    public function __construct(public readonly string $id, public readonly ?string $admin_label = NULL, public readonly ?string $deriver = NULL)
    {
    }
}
