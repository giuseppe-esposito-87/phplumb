<?php
namespace Drupal\Core\Validation;

use Drupal\Core\Plugin\DefaultPluginManager;
use Drupal\Core\Validation\Attribute\Constraint;

class ConstraintManager extends DefaultPluginManager
{
    public function __construct(\Traversable $namespaces, $cache_backend, $module_handler)
    {
        parent::__construct('Plugin/Validation/Constraint', $namespaces, $module_handler, NULL, Constraint::class, 'Drupal\Core\Validation\Annotation\Constraint');
    }
}
