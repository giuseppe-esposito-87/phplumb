<?php
namespace Drupal\Core\Entity;

interface EntityTypeManagerInterface
{
    public function getStorage($entity_type_id);
}
