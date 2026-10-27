<?php
namespace Drupal\gadget;

use Drupal\Core\Plugin\DefaultPluginManager;
use Drupal\gadget\Attribute\GadgetElement;

class GadgetElementManager extends DefaultPluginManager
{
    public function __construct(\Traversable $namespaces, $module_handler)
    {
        parent::__construct('Plugin/GadgetElement', $namespaces, $module_handler, NULL, GadgetElement::class);
    }
}
