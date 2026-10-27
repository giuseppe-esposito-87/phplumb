<?php
namespace Drupal\mymodule;

use Drupal\Component\Plugin\PluginManagerInterface;
use Drupal\Core\Block\BlockManager;
use Drupal\Core\Field\BaseFieldDefinition;

class PluginCalls
{
    public function run(BlockManager $blocks, PluginManagerInterface $any): void
    {
        $blocks->createInstance('my_block');
        \Drupal::service('plugin.manager.block')->getDefinition('my_block');
        $any->createInstance('my_block');
        $blocks->hasDefinition('nothing_here');
        BaseFieldDefinition::create('string')->addConstraint('UniqueField');
    }

    public function entity(\Drupal\Core\Entity\EntityTypeInterface $type): void
    {
        $type->addConstraint('UniqueField');
    }
}
