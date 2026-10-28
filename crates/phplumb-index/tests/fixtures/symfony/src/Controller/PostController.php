<?php

namespace App\Controller;

use Symfony\Component\Routing\Attribute\Route;

#[Route('/post', name: 'post_')]
class PostController extends CrudController
{
}
