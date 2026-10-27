<?php
namespace Drupal\mymodule\Controller;

use Symfony\Component\Routing\Attribute\Route;

class AttributeController
{
    #[Route(path: '/attribute', name: 'mymodule.attribute_route')]
    public function page()
    {
    }
}
