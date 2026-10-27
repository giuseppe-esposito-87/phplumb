<?php
namespace Drupal\Core\Block;

use Drupal\Component\Plugin\PluginManagerInterface;
use Drupal\Core\Block\Attribute\Block;
use Drupal\Core\Plugin\DefaultPluginManager;

interface BlockManagerInterface extends PluginManagerInterface
{
}

class BlockManager extends DefaultPluginManager implements BlockManagerInterface
{
    public function __construct(\Traversable $namespaces, $cache_backend, $module_handler)
    {
        parent::__construct('Plugin/Block', $namespaces, $module_handler, BlockPluginInterface::class, Block::class, 'Drupal\Core\Block\Annotation\Block');
    }
}
