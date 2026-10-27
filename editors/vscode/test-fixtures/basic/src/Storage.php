<?php
namespace App;
function storage(\Drupal\Core\Entity\EntityTypeManagerInterface $etm) { $etm->getStorage('basic_thing'); }
