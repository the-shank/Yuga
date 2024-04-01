<h2>Potential use-after-free!</h2>
<p>src/lib.rs:477:5: 477:60</p>
<pre style="background-color:#2b303b;"><code class="language-rust">
<span style="color:#b48ead;">pub fn </span><span style="color:#8fa1b3;">peek_lru</span><span style="color:#c0c5ce;">&lt;</span><span style="color:#b48ead;">&#39;a</span><span style="color:#c0c5ce;">&gt;(&amp;&#39;_ </span><span style="color:#bf616a;">self</span><span style="color:#c0c5ce;">) -&gt; Option&lt;(&amp;</span><span style="color:#b48ead;">&#39;a</span><span style="color:#c0c5ce;"> K, &amp;</span><span style="color:#b48ead;">&#39;a</span><span style="color:#c0c5ce;"> V)&gt;
</span>
</code></pre>
<p><code>*(*(*(self).tail).prev).0</code> is of type <code>K</code> and outlives the lifetime corresponding to <code>'_</code>,</p>
<p>It is (probably) returned as <code>*(ret.0.0)</code> which is of type <code>K</code>, and outlives the lifetime corresponding to <code>'a</code>, . Here, <code>ret</code> denotes the value returned by the function.</p>
<p>The latter can be longer than the former, which could lead to use-after-free!</p>
<p><strong>Detailed report:</strong></p>
<p><code>self</code> is of type <code>LruCache&lt;K, V, S&gt;</code></p>
<pre style="background-color:#2b303b;"><code class="language-rust">
<span style="color:#b48ead;">pub struct </span><span style="color:#c0c5ce;">LruCache&lt;K, V, S = DefaultHasher&gt; {
</span><span style="color:#c0c5ce;">    </span><span style="color:#bf616a;">map</span><span style="color:#c0c5ce;">: HashMap&lt;KeyRef&lt;K&gt;, Box&lt;LruEntry&lt;K, V&gt;&gt;, S&gt;,
</span><span style="color:#c0c5ce;">    </span><span style="color:#bf616a;">cap</span><span style="color:#c0c5ce;">: </span><span style="color:#b48ead;">usize</span><span style="color:#c0c5ce;">,
</span><span style="color:#c0c5ce;">
</span><span style="color:#c0c5ce;">    </span><span style="color:#65737e;">// head and tail are sigil nodes to faciliate inserting entries
</span><span style="color:#c0c5ce;">    </span><span style="color:#bf616a;">head</span><span style="color:#c0c5ce;">: *</span><span style="color:#b48ead;">mut </span><span style="color:#c0c5ce;">LruEntry&lt;K, V&gt;,
</span><span style="color:#c0c5ce;">    </span><span style="color:#bf616a;">tail</span><span style="color:#c0c5ce;">: *</span><span style="color:#b48ead;">mut </span><span style="color:#c0c5ce;">LruEntry&lt;K, V&gt;,
</span><span style="color:#c0c5ce;">}
</span>
</code></pre>
<p><code>LruCache&lt;K, V, S&gt;</code> has a custom <code>Drop</code> implementation.</p>
<pre style="background-color:#2b303b;"><code class="language-rust">
<span style="color:#b48ead;">fn </span><span style="color:#8fa1b3;">drop</span><span style="color:#c0c5ce;">(&amp;</span><span style="color:#b48ead;">mut </span><span style="color:#bf616a;">self</span><span style="color:#c0c5ce;">) {
</span><span style="color:#c0c5ce;">        </span><span style="color:#bf616a;">self</span><span style="color:#c0c5ce;">.map.</span><span style="color:#96b5b4;">values_mut</span><span style="color:#c0c5ce;">().</span><span style="color:#96b5b4;">for_each</span><span style="color:#c0c5ce;">(|</span><span style="color:#bf616a;">e</span><span style="color:#c0c5ce;">| </span><span style="color:#b48ead;">unsafe </span><span style="color:#c0c5ce;">{
</span><span style="color:#c0c5ce;">            ptr::drop_in_place(e.key.</span><span style="color:#96b5b4;">as_mut_ptr</span><span style="color:#c0c5ce;">());
</span><span style="color:#c0c5ce;">            ptr::drop_in_place(e.val.</span><span style="color:#96b5b4;">as_mut_ptr</span><span style="color:#c0c5ce;">());
</span><span style="color:#c0c5ce;">        });
</span><span style="color:#c0c5ce;">        </span><span style="color:#65737e;">// We rebox the head/tail, and because these are maybe-uninit
</span><span style="color:#c0c5ce;">        </span><span style="color:#65737e;">// they do not have the absent k/v dropped.
</span><span style="color:#c0c5ce;">        </span><span style="color:#b48ead;">unsafe </span><span style="color:#c0c5ce;">{
</span><span style="color:#c0c5ce;">            </span><span style="color:#b48ead;">let</span><span style="color:#c0c5ce;"> _head = *Box::from_raw(</span><span style="color:#bf616a;">self</span><span style="color:#c0c5ce;">.head);
</span><span style="color:#c0c5ce;">            </span><span style="color:#b48ead;">let</span><span style="color:#c0c5ce;"> _tail = *Box::from_raw(</span><span style="color:#bf616a;">self</span><span style="color:#c0c5ce;">.tail);
</span><span style="color:#c0c5ce;">        }
</span><span style="color:#c0c5ce;">    }
</span>
</code></pre>
<p><code>*(self).tail</code> is of type <code>*mut LruEntry&lt;K, V&gt;</code></p>
<pre style="background-color:#2b303b;"><code class="language-rust">
<span style="color:#b48ead;">struct </span><span style="color:#c0c5ce;">LruEntry&lt;K, V&gt; {
</span><span style="color:#c0c5ce;">    </span><span style="color:#bf616a;">key</span><span style="color:#c0c5ce;">: mem::MaybeUninit&lt;K&gt;,
</span><span style="color:#c0c5ce;">    </span><span style="color:#bf616a;">val</span><span style="color:#c0c5ce;">: mem::MaybeUninit&lt;V&gt;,
</span><span style="color:#c0c5ce;">    </span><span style="color:#bf616a;">prev</span><span style="color:#c0c5ce;">: *</span><span style="color:#b48ead;">mut </span><span style="color:#c0c5ce;">LruEntry&lt;K, V&gt;,
</span><span style="color:#c0c5ce;">    </span><span style="color:#bf616a;">next</span><span style="color:#c0c5ce;">: *</span><span style="color:#b48ead;">mut </span><span style="color:#c0c5ce;">LruEntry&lt;K, V&gt;,
</span><span style="color:#c0c5ce;">}
</span>
</code></pre>
<p><code>*(*(self).tail).prev</code> is of type <code>*mut LruEntry&lt;K, V&gt;</code></p>
<p><code>ret</code> is of type <code>Option&lt;(&amp;'a K, &amp;'a V)&gt;</code></p>
<p>Here is the full body of the function:</p>
<pre style="background-color:#2b303b;"><code class="language-rust">
<span style="color:#b48ead;">pub fn </span><span style="color:#8fa1b3;">peek_lru</span><span style="color:#c0c5ce;">&lt;</span><span style="color:#b48ead;">&#39;a</span><span style="color:#c0c5ce;">&gt;(&amp;&#39;_ </span><span style="color:#bf616a;">self</span><span style="color:#c0c5ce;">) -&gt; Option&lt;(&amp;</span><span style="color:#b48ead;">&#39;a</span><span style="color:#c0c5ce;"> K, &amp;</span><span style="color:#b48ead;">&#39;a</span><span style="color:#c0c5ce;"> V)&gt;{
</span><span style="color:#c0c5ce;">        </span><span style="color:#b48ead;">if </span><span style="color:#bf616a;">self</span><span style="color:#c0c5ce;">.</span><span style="color:#96b5b4;">is_empty</span><span style="color:#c0c5ce;">() {
</span><span style="color:#c0c5ce;">            </span><span style="color:#b48ead;">return </span><span style="color:#c0c5ce;">None;
</span><span style="color:#c0c5ce;">        }
</span><span style="color:#c0c5ce;">
</span><span style="color:#c0c5ce;">        </span><span style="color:#b48ead;">let </span><span style="color:#c0c5ce;">(key, val);
</span><span style="color:#c0c5ce;">        </span><span style="color:#b48ead;">unsafe </span><span style="color:#c0c5ce;">{
</span><span style="color:#c0c5ce;">            </span><span style="color:#b48ead;">let</span><span style="color:#c0c5ce;"> node = (*</span><span style="color:#bf616a;">self</span><span style="color:#c0c5ce;">.tail).prev;
</span><span style="color:#c0c5ce;">            key = &amp;(*(*node).key.</span><span style="color:#96b5b4;">as_ptr</span><span style="color:#c0c5ce;">()) as &amp;K;
</span><span style="color:#c0c5ce;">            val = &amp;(*(*node).val.</span><span style="color:#96b5b4;">as_ptr</span><span style="color:#c0c5ce;">()) as &amp;V;
</span><span style="color:#c0c5ce;">        }
</span><span style="color:#c0c5ce;">
</span><span style="color:#c0c5ce;">        Some((key, val))
</span><span style="color:#c0c5ce;">    }
</span>
</code></pre>
