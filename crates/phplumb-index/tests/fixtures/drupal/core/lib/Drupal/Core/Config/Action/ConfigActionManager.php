<?php
namespace Drupal\Core\Config\Action;

use Drupal\Core\Plugin\DefaultPluginManager;

class ConfigActionManager extends DefaultPluginManager
{
    public function __construct(\Traversable $namespaces, $module_handler)
    {
        parent::__construct('Plugin/ConfigAction', $namespaces, $module_handler, NULL, ConfigAction::class);
    }

    public function createInstance($plugin_id, array $configuration = [])
    {
    }
}
