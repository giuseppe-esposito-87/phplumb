<?php
namespace Drupal\Core\Field;

use Drupal\Core\Field\Attribute\FieldFormatter;
use Drupal\Core\Plugin\DefaultPluginManager;

class FormatterPluginManager extends DefaultPluginManager
{
    public function __construct(\Traversable $namespaces, $cache_backend, $module_handler)
    {
        parent::__construct('Plugin/Field/FieldFormatter', $namespaces, $module_handler, FormatterInterface::class, FieldFormatter::class, 'Drupal\Core\Field\Annotation\FieldFormatter');
    }
}
