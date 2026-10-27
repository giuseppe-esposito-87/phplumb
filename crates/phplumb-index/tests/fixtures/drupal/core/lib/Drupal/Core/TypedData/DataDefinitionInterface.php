<?php
namespace Drupal\Core\TypedData;

interface DataDefinitionInterface
{
    public function addConstraint($constraint_name, $options = NULL);
}
