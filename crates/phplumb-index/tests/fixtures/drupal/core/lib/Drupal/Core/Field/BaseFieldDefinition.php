<?php
namespace Drupal\Core\Field;

use Drupal\Core\TypedData\DataDefinitionInterface;

class BaseFieldDefinition implements DataDefinitionInterface
{
    /**
     * @return static
     */
    public static function create($type)
    {
    }

    /**
     * @return $this
     */
    public function setDisplayOptions($display_context, array $options)
    {
    }

    /**
     * @return $this
     */
    public function addConstraint($constraint_name, $options = NULL)
    {
    }
}
