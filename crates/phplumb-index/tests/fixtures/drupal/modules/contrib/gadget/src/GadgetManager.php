<?php
namespace Drupal\gadget;

use Drupal\Core\Plugin\DefaultPluginManager;

class GadgetManager extends DefaultPluginManager
{
    public function __construct($type, \Traversable $namespaces, $module_handler)
    {
        parent::__construct("Plugin/gadget/$type", $namespaces, $module_handler);
    }
}
