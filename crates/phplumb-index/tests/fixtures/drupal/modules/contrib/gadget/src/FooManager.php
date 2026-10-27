<?php
namespace Drupal\gadget;

use Drupal\Core\Plugin\DefaultPluginManager;

class FooManager extends DefaultPluginManager
{
    public function __construct(\Traversable $namespaces, $module_handler)
    {
        parent::__construct('Plugin/Foo', $namespaces, $module_handler, 'Drupal\gadget\FooInterface', 'Drupal\gadget\Annotation\Foo');
    }
}
