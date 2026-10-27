<?php
namespace Drupal\Core\Entity;

interface EntityTypeInterface
{
    public function addConstraint($constraint_name, $options = NULL);
}
