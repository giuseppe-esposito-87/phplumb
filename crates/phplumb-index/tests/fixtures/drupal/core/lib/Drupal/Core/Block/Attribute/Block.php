<?php
namespace Drupal\Core\Block\Attribute;

#[\Attribute(\Attribute::TARGET_CLASS)]
class Block extends \Drupal\Component\Plugin\Attribute\Plugin
{
    public function __construct(public readonly string $id, public readonly ?string $admin_label = NULL, public readonly ?string $deriver = NULL)
    {
    }
}
