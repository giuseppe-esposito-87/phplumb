<?php
namespace Drupal\mymodule\Quiet;

use Drupal\Core\Block\BlockManager;
use Symfony\Component\DependencyInjection\ContainerInterface;

class SoftLookups
{
    public function run(ContainerInterface $container, BlockManager $blocks)
    {
        $container->get('optional.service', ContainerInterface::NULL_ON_INVALID_REFERENCE);
        $blocks->getDefinition('optional_block', false);
    }
}
